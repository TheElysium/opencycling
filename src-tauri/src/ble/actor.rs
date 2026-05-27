use btleplug::api::{Central, Peripheral, ScanFilter, WriteType};
use btleplug::platform;
use btleplug::platform::Adapter;
use futures::StreamExt;
use tauri::Emitter;
use tokio::sync::oneshot::Sender;
use crate::ble::types::{BleActor, BleCommand, BleMetrics, DeviceInfo, DeviceKind, ParsedNotifications};
use crate::errors::AppError;
use uuid::Uuid;
use crate::ble::ftms::parse_indoor_bike_data;
use crate::ble::hrs::parse_heart_rate_measurement;

// UUIDs standard Bluetooth SIG
const FITNESS_MACHINE_SERVICE: Uuid = Uuid::from_u128(0x00001826_0000_1000_8000_00805f9b34fb);
const HEART_RATE_SERVICE: Uuid = Uuid::from_u128(0x0000180d_0000_1000_8000_00805f9b34fb);
const INDOOR_BIKE_DATA:   Uuid = Uuid::from_u128(0x00002ad2_0000_1000_8000_00805f9b34fb);
const HEART_RATE_MEAS:    Uuid = Uuid::from_u128(0x00002a37_0000_1000_8000_00805f9b34fb);
const FTMS_CONTROL_POINT: Uuid = Uuid::from_u128(0x00002ad9_0000_1000_8000_00805f9b34fb);
const KEEP_ALIVE_TICK: u64 = 10;
const METRICS_TICK: u64 = 1;

impl BleActor {
    pub async fn run(mut self) {
        let mut keep_alive = tokio::time::interval(tokio::time::Duration::from_secs(KEEP_ALIVE_TICK));
        let mut metrics_ticker = tokio::time::interval(tokio::time::Duration::from_secs(METRICS_TICK));
        loop {
            tokio::select! {
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        None => {break;},
                        Some(cmd) => match cmd {
                            BleCommand::Scan { reply } => handle_scan(reply, &self.adapter).await,
                            BleCommand::ConnectTrainer { device_id, reply } => self.handle_connect_trainer(device_id, reply).await,
                            BleCommand::ConnectHrm { device_id, reply } => self.handle_connect_hrm(device_id, reply).await,
                            BleCommand::SetTargetPower { watts } => self.last_target_w = Some(watts),
                        }
                    }
                }
                _keep_alive = keep_alive.tick() => {
                    if let (Some(trainer), Some(watts)) = (&self.trainer, self.last_target_w) {
                        let _ = send_erg(trainer, watts).await;
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
                    }
                }
                _metrics_ticker = metrics_ticker.tick() => {
                    self.emit_metrics()
                }
            }
        }
    }

    async fn handle_connect_trainer(
        &mut self,
        device_id: String,
        reply: Sender<Result<(), AppError>>,
    ) {
        let _ = reply.send(self.do_connect_trainer(device_id).await);
    }

    async fn connect_peripheral(&self, device_id: String, characteristic_uuid: Uuid) -> Result<platform::Peripheral, AppError> {
        let peripherals = self.adapter.peripherals()
            .await
            .map_err(|e| AppError::BLEConnectError(e.to_string()))?;

        let trainer = peripherals
            .into_iter()
            .find(|p| p.id().to_string() == device_id)
            .ok_or_else(|| AppError::DeviceNotFound(device_id.clone()))?;

        trainer.connect().await.map_err(|e| AppError::BLEConnectError(e.to_string()))?;
        trainer.discover_services().await.map_err(|e| AppError::BLEConnectError(e.to_string()))?;

        let characteristic = trainer.characteristics()
            .into_iter().find(|c| c.uuid == characteristic_uuid)
            .ok_or_else(|| AppError::CharacteristicNotFound(characteristic_uuid.to_string()))?;

        trainer.subscribe(&characteristic)
            .await
            .map_err(|e| AppError::BLEConnectError(e.to_string()))?;

        Ok(trainer)
    }

    async fn do_connect_trainer(&mut self, device_id: String) -> Result<(), AppError> {
        let trainer = self.connect_peripheral(device_id, INDOOR_BIKE_DATA).await?;
        request_control(&trainer).await?;
        let mut stream = trainer.notifications()
            .await
            .map_err(|e| AppError::BLEConnectError(e.to_string()))?;
        let tx = self.notif_tx.clone();
        tokio::spawn(async move {
            while let Some(notification) = stream.next().await {
                if notification.uuid == INDOOR_BIKE_DATA {
                    if let Ok(data) = parse_indoor_bike_data(&notification.value) {
                        let _ = tx
                            .send(ParsedNotifications::TrainerData { power_w: data.instantaneous_power_w, cadence_rpm: data.instantaneous_cadence_rpm })
                            .await;
                    }

                }
            }
        });
        self.trainer = Some(trainer);
        Ok(())
    }

    async fn handle_connect_hrm(
        &mut self,
        device_id: String,
        reply: Sender<Result<(), AppError>>
    ) {
        let _ = reply.send(self.do_connect_hrm(device_id).await);
    }

    async fn do_connect_hrm(&mut self, device_id: String) -> Result<(), AppError>{
        let hrm = self
            .connect_peripheral(device_id, HEART_RATE_MEAS)
            .await?;
        let mut stream = hrm.notifications()
            .await
            .map_err(|e| AppError::BLEConnectError(e.to_string()))?;
        let tx = self.notif_tx.clone();
        tokio::spawn(async move {
            while let Some(notification) = stream.next().await {
                if notification.uuid == HEART_RATE_MEAS {
                    if let Ok(data) = parse_heart_rate_measurement(&notification.value) {
                        let _ = tx
                            .send(ParsedNotifications::HRMData { hr_bpm: data.hr_bpm })
                            .await;
                    }

                }
            }
        });
        self.hrm = Some(hrm);
        Ok(())
    }

    fn emit_metrics(&self) {
        let ble_metric = BleMetrics{
            power_w: self.last_power_w,
            hr_bpm: self.last_hr_bpm,
            cadence_rpm: self.last_cadence_rpm,
        };
        let _ = self.app_handle.emit("ble_metrics", &ble_metric);
    }
}

async fn handle_scan(reply: Sender<Result<Vec<DeviceInfo>, AppError>>, adapter: &Adapter)  {
    let _ = reply.send(do_scan(adapter).await);
}

async fn do_scan(adapter: &Adapter) -> Result<Vec<DeviceInfo>, AppError> {
    adapter
        .start_scan(ScanFilter::default())
        .await
        .map_err(|e| AppError::BLEScanError(e.to_string()))?;

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    adapter
        .stop_scan()
        .await
        .map_err(|e| AppError::BLEScanError(e.to_string()))?;


    let peripherals = adapter.peripherals()
        .await
        .map_err(|e| AppError::BLEScanError(e.to_string()))?;

    let mut devices_info = Vec::new();

    for peripheral in peripherals {
        match peripheral.properties()
            .await
            .map_err(|e| AppError::BLEScanError(e.to_string()))?
        {
            None => { continue }
            Some(prop) => {
                let Some(ref name) = prop.local_name else {continue};
                devices_info.push(DeviceInfo{
                    id: peripheral.id().to_string(),
                    name: name.to_string(),
                    kind: get_device_kind(prop.services, name),
                })
            }
        }
    }

    Ok(devices_info)
}

fn get_device_kind(services: Vec<Uuid>, name: &str) -> Option<DeviceKind> {
    if services.contains(&FITNESS_MACHINE_SERVICE) {
        return Some(DeviceKind::Trainer);
    }
    if services.contains(&HEART_RATE_SERVICE) {
        return Some(DeviceKind::Hrm)
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
        return Some(DeviceKind::Hrm);
    }

    None
}

async fn request_control(trainer: &platform::Peripheral) -> Result<(), AppError> {
    let control_point = trainer.characteristics()
        .into_iter()
        .find(|c|c.uuid == FTMS_CONTROL_POINT)
        .ok_or_else(|| AppError::BLEConnectError("Failed to find FTMS_CONTROL_POINT characteristic".to_string()))?;

    trainer.subscribe(&control_point)
        .await
        .map_err(|e| AppError::BLEConnectError(e.to_string()))?;

    let payload = [0x00u8];
    trainer.write(&control_point, &payload, WriteType::WithResponse)
        .await
        .map_err(|e| AppError::BLECommandError(e.to_string()))
}

async fn send_erg(trainer: &platform::Peripheral, watts: i16) -> Result<(), AppError> {
    let control_point = trainer.characteristics()
        .into_iter()
        .find(|c|c.uuid == FTMS_CONTROL_POINT)
        .ok_or_else(|| AppError::BLECommandError("Failed to find FTMS_CONTROL_POINT characteristic".to_string()))?;

    let watts_le = watts.to_le_bytes();
    let payload = [0x05, watts_le[0], watts_le[1]];

    trainer.write(&control_point, &payload, WriteType::WithResponse)
        .await
        .map_err(|e| AppError::BLECommandError(e.to_string()))
}