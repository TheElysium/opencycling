use crate::ble::ftms::parse_indoor_bike_data;
use crate::ble::hrs::parse_heart_rate_measurement;
use crate::ble::types::DeviceKind::Trainer;
use crate::ble::types::{
    BleActor, BleCommand, BleError, BleMetrics, DeviceInfo, DeviceKind, ParsedNotifications,
};
use crate::errors::AppError;
use btleplug::api::{Central, CentralEvent, Peripheral, ScanFilter, WriteType};
use btleplug::platform;
use btleplug::platform::Adapter;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use tauri::Emitter;
use tokio::sync::oneshot::Sender;
use tracing::{error, info};
use uuid::Uuid;
use DeviceKind::Hrm;

// UUIDs standard Bluetooth SIG
const FITNESS_MACHINE_SERVICE: Uuid = Uuid::from_u128(0x00001826_0000_1000_8000_00805f9b34fb);
const HEART_RATE_SERVICE: Uuid = Uuid::from_u128(0x0000180d_0000_1000_8000_00805f9b34fb);
const INDOOR_BIKE_DATA: Uuid = Uuid::from_u128(0x00002ad2_0000_1000_8000_00805f9b34fb);
const HEART_RATE_MEAS: Uuid = Uuid::from_u128(0x00002a37_0000_1000_8000_00805f9b34fb);
const FTMS_CONTROL_POINT: Uuid = Uuid::from_u128(0x00002ad9_0000_1000_8000_00805f9b34fb);
const KEEP_ALIVE_TICK: u64 = 10;
const METRICS_TICK: u64 = 1;

impl BleActor {
    pub async fn run(mut self) {
        // Subscribe to adapter-level events to detect disconnections.
        // Falls back to a never-resolving stream if subscription fails.
        let mut adapter_events: Pin<Box<dyn Stream<Item = CentralEvent> + Send>> =
            self.adapter.events().await.unwrap_or_else(|e| {
                error!(
                    "failed to subscribe to adapter events, disconnection detection disabled: {e}"
                );
                Box::pin(futures::stream::pending())
            });

        let mut keep_alive =
            tokio::time::interval(tokio::time::Duration::from_secs(KEEP_ALIVE_TICK));
        let mut metrics_ticker =
            tokio::time::interval(tokio::time::Duration::from_secs(METRICS_TICK));

        loop {
            tokio::select! {
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        None => {
                            info!("BleActor shutting down");
                            break;
                        }
                        Some(cmd) => match cmd {
                            BleCommand::Scan { reply } => handle_scan(reply, &self.adapter).await,
                            BleCommand::ConnectTrainer { device_id, reply } => self.handle_connect_trainer(device_id, reply).await,
                            BleCommand::ConnectHrm { device_id, reply } => self.handle_connect_hrm(device_id, reply).await,
                            BleCommand::SetTargetPower { watts } => {
                                self.last_target_w = Some(watts);
                                if let Some(trainer) = &self.trainer {
                                    if let Err(e) = send_erg(trainer, watts).await {
                                        error!("ERG set_target_power failed: {e}");
                                    }
                                }
                            }
                        }
                    }
                }
                _keep_alive = keep_alive.tick() => {
                    if let (Some(trainer), Some(watts)) = (&self.trainer, self.last_target_w) {
                        if let Err(e) = send_erg(trainer, watts).await {
                            error!("ERG keep-alive failed: {e}");
                        }
                    }
                }
                Some(notifications) = self.notif_rx.recv() => {
                    match notifications {
                        ParsedNotifications::TrainerData{ power_w, cadence_rpm } => {
                            self.last_power_w = power_w;
                            self.last_cadence_rpm = cadence_rpm;
                        }
                        ParsedNotifications::HRMData{ hr_bpm } => {
                            self.last_hr_bpm = Some(hr_bpm);
                        }
                        ParsedNotifications::ParseError { device_kind, error } => {
                            let device = match device_kind {
                                Trainer => "trainer",
                                Hrm => "hrm",
                            };
                            let _ = self.app_handle.emit("ble_error", BleError {
                                device: device.into(),
                                message: error.to_string(),
                            });
                        }
                    }
                }
                _metrics_ticker = metrics_ticker.tick() => {
                    self.emit_metrics()
                }
                Some(event) = adapter_events.next() => {
                    self.handle_adapter_event(event);
                }
            }
        }
    }

    fn handle_adapter_event(&mut self, event: CentralEvent) {
        let CentralEvent::DeviceDisconnected(id) = event else {
            return;
        };

        if self.trainer.as_ref().map(|p| p.id() == id).unwrap_or(false) {
            info!("trainer disconnected");
            self.trainer = None;
            self.last_power_w = None;
            self.last_cadence_rpm = None;
            self.last_target_w = None;
            let _ = self.app_handle.emit("ble_disconnected", "trainer");
        } else if self.hrm.as_ref().map(|p| p.id() == id).unwrap_or(false) {
            info!("hrm disconnected");
            self.hrm = None;
            self.last_hr_bpm = None;
            let _ = self.app_handle.emit("ble_disconnected", "hrm");
        }
    }

    async fn handle_connect_trainer(
        &mut self,
        device_id: String,
        reply: Sender<Result<(), AppError>>,
    ) {
        let result = self.do_connect_trainer(device_id).await;
        if let Err(ref e) = result {
            error!("connect_trainer failed: {e}");
        }
        let _ = reply.send(result);
    }

    async fn connect_peripheral(
        &self,
        device_id: String,
        characteristic_uuid: Uuid,
    ) -> Result<platform::Peripheral, AppError> {
        let peripherals = self
            .adapter
            .peripherals()
            .await
            .map_err(|e| AppError::BLEConnectError(e.to_string()))?;

        let trainer = peripherals
            .into_iter()
            .find(|p| p.id().to_string() == device_id)
            .ok_or_else(|| AppError::DeviceNotFound(device_id.clone()))?;

        trainer
            .connect()
            .await
            .map_err(|e| AppError::BLEConnectError(e.to_string()))?;
        trainer
            .discover_services()
            .await
            .map_err(|e| AppError::BLEConnectError(e.to_string()))?;

        let characteristic = trainer
            .characteristics()
            .into_iter()
            .find(|c| c.uuid == characteristic_uuid)
            .ok_or_else(|| AppError::CharacteristicNotFound(characteristic_uuid.to_string()))?;

        trainer
            .subscribe(&characteristic)
            .await
            .map_err(|e| AppError::BLEConnectError(e.to_string()))?;

        Ok(trainer)
    }

    async fn do_connect_trainer(&mut self, device_id: String) -> Result<(), AppError> {
        if let Some(handle) = self.trainer_task.take() {
            handle.abort();
            self.trainer = None;
        }
        info!("connecting trainer: {device_id}");
        let trainer = self.connect_peripheral(device_id, INDOOR_BIKE_DATA).await?;
        request_control(&trainer).await?;
        // The btleplug notification stream cannot be polled directly inside select!
        // because holding a mutable reference to the stream would conflict with
        // &mut self on the other branches. Instead we spawn a dedicated task that
        // owns the stream and forwards parsed values over the notif channel.
        let mut stream = trainer
            .notifications()
            .await
            .map_err(|e| AppError::BLEConnectError(e.to_string()))?;
        let tx = self.notif_tx.clone();
        let handle = tokio::spawn(async move {
            while let Some(notification) = stream.next().await {
                if notification.uuid == INDOOR_BIKE_DATA {
                    match parse_indoor_bike_data(&notification.value) {
                        Ok(data) => {
                            let _ = tx.try_send(ParsedNotifications::TrainerData {
                                power_w: data.instantaneous_power_w,
                                cadence_rpm: data.instantaneous_cadence_rpm,
                            });
                        }
                        Err(e) => {
                            let _ = tx.try_send(ParsedNotifications::ParseError {
                                device_kind: Trainer,
                                error: e,
                            });
                        }
                    }
                }
            }
        });
        self.trainer_task = Some(handle.abort_handle());
        self.trainer = Some(trainer);
        info!("trainer connected");
        Ok(())
    }

    async fn handle_connect_hrm(&mut self, device_id: String, reply: Sender<Result<(), AppError>>) {
        let result = self.do_connect_hrm(device_id).await;
        if let Err(ref e) = result {
            error!("connect_hrm failed: {e}");
        }
        let _ = reply.send(result);
    }

    async fn do_connect_hrm(&mut self, device_id: String) -> Result<(), AppError> {
        if let Some(handle) = self.hrm_task.take() {
            handle.abort();
            self.hrm = None;
        }
        info!("connecting hrm: {device_id}");
        let hrm = self.connect_peripheral(device_id, HEART_RATE_MEAS).await?;
        // Same pattern as the trainer task: dedicated task owns the stream,
        // forwards parsed HR values over the shared notif channel.
        let mut stream = hrm
            .notifications()
            .await
            .map_err(|e| AppError::BLEConnectError(e.to_string()))?;
        let tx = self.notif_tx.clone();
        let handle = tokio::spawn(async move {
            while let Some(notification) = stream.next().await {
                if notification.uuid == HEART_RATE_MEAS {
                    match parse_heart_rate_measurement(&notification.value) {
                        Ok(data) => {
                            let _ = tx.try_send(ParsedNotifications::HRMData {
                                hr_bpm: data.hr_bpm,
                            });
                        }
                        Err(e) => {
                            let _ = tx.try_send(ParsedNotifications::ParseError {
                                device_kind: Hrm,
                                error: e,
                            });
                        }
                    }
                }
            }
        });
        self.hrm_task = Some(handle.abort_handle());
        self.hrm = Some(hrm);
        info!("hrm connected");
        Ok(())
    }

    fn emit_metrics(&self) {
        if self.trainer.is_none() && self.hrm.is_none() {
            return;
        }
        let ble_metric = BleMetrics {
            power_w: self.last_power_w,
            hr_bpm: self.last_hr_bpm,
            cadence_rpm: self.last_cadence_rpm,
        };
        let _ = self.app_handle.emit("ble_metrics", &ble_metric);
    }
}

async fn handle_scan(reply: Sender<Result<Vec<DeviceInfo>, AppError>>, adapter: &Adapter) {
    let result = do_scan(adapter).await;
    if let Err(ref e) = result {
        error!("scan failed: {e}");
    }
    let _ = reply.send(result);
}

async fn do_scan(adapter: &Adapter) -> Result<Vec<DeviceInfo>, AppError> {
    info!("BLE scan started");
    // stop_scan() first flushes the Windows BLE cache so stale (powered-off)
    // devices from a previous scan no longer appear in peripherals().
    let _ = adapter.stop_scan().await;
    adapter
        .start_scan(ScanFilter::default())
        .await
        .map_err(|e| AppError::BLEScanError(e.to_string()))?;

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    adapter
        .stop_scan()
        .await
        .map_err(|e| AppError::BLEScanError(e.to_string()))?;

    let peripherals = adapter
        .peripherals()
        .await
        .map_err(|e| AppError::BLEScanError(e.to_string()))?;

    let mut devices_info = Vec::new();

    for peripheral in peripherals {
        match peripheral
            .properties()
            .await
            .map_err(|e| AppError::BLEScanError(e.to_string()))?
        {
            None => continue,
            Some(prop) => {
                let Some(ref name) = prop.local_name else {
                    continue;
                };
                devices_info.push(DeviceInfo {
                    id: peripheral.id().to_string(),
                    name: name.to_string(),
                    kind: get_device_kind(prop.services, name),
                })
            }
        }
    }
    info!("BLE scan complete: {} device(s) found", devices_info.len());
    Ok(devices_info)
}

fn get_device_kind(services: Vec<Uuid>, name: &str) -> Option<DeviceKind> {
    if services.contains(&FITNESS_MACHINE_SERVICE) {
        return Some(DeviceKind::Trainer);
    }
    if services.contains(&HEART_RATE_SERVICE) {
        return Some(Hrm);
    }

    let name_lower = name.to_lowercase();
    if name_lower.contains("kickr")
        || name_lower.contains("neo")
        || name_lower.contains("hammer")
        || name_lower.contains("trainer")
        || name_lower.contains("flux")
        || name_lower.contains("d500")
    {
        return Some(DeviceKind::Trainer);
    }
    if name_lower.contains("hrm")
        || name_lower.contains("heart rate")
        || name_lower.contains("polar h")
        || name_lower.contains("wahoo tickr")
    {
        return Some(Hrm);
    }

    None
}

async fn request_control(trainer: &platform::Peripheral) -> Result<(), AppError> {
    let control_point = trainer
        .characteristics()
        .into_iter()
        .find(|c| c.uuid == FTMS_CONTROL_POINT)
        .ok_or_else(|| {
            AppError::BLEConnectError(
                "Failed to find FTMS_CONTROL_POINT characteristic".to_string(),
            )
        })?;

    trainer
        .subscribe(&control_point)
        .await
        .map_err(|e| AppError::BLEConnectError(e.to_string()))?;

    let payload = [0x00u8];
    trainer
        .write(&control_point, &payload, WriteType::WithResponse)
        .await
        .map_err(|e| AppError::BLECommandError(e.to_string()))
}

async fn send_erg(trainer: &platform::Peripheral, watts: i16) -> Result<(), AppError> {
    let control_point = trainer
        .characteristics()
        .into_iter()
        .find(|c| c.uuid == FTMS_CONTROL_POINT)
        .ok_or_else(|| {
            AppError::BLECommandError(
                "Failed to find FTMS_CONTROL_POINT characteristic".to_string(),
            )
        })?;

    let watts_le = watts.to_le_bytes();
    let payload = [0x05, watts_le[0], watts_le[1]];

    trainer
        .write(&control_point, &payload, WriteType::WithResponse)
        .await
        .map_err(|e| AppError::BLECommandError(e.to_string()))
}
