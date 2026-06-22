use crate::ble::ftms::{build_set_target_power_command, parse_indoor_bike_data};
use crate::ble::hrs::parse_heart_rate_measurement;
use crate::ble::types::DeviceKind::Trainer;
use crate::ble::types::{
    BleActor, BleCommand, BleError, BleEvent, BleMetrics, BleReconnect, DeviceInfo, DeviceKind,
    ParsedNotifications, ReconnectMsg,
};
use crate::errors::AppError;
use btleplug::api::{Central, CentralEvent, Peripheral, ScanFilter, WriteType};
use btleplug::platform;
use btleplug::platform::Adapter;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc::Sender as MpscSender;
use tokio::sync::oneshot::Sender;
use tokio::sync::Mutex;
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
// Reconnect cadence + safety cap: ~40 attempts at 3 s ≈ 2 min before giving up.
const RECONNECT_INTERVAL_S: u64 = 3;
const RECONNECT_MAX_ATTEMPTS: u32 = 40;
// Per-attempt timeout: a WinRT scan/probe can stall, so each attempt is bounded so
// the 3 s cadence holds rather than drifting (issue 17).
const RECONNECT_ATTEMPT_TIMEOUT_S: u64 = 3;
const RECONNECT_SCAN_DWELL_MS: u64 = 1500;
// Consecutive ERG write failures before the trainer is declared lost. WinRT lags ~40 s
// on DeviceDisconnected and is_connected(), so failed writes are the timely signal; 2
// in a row (at most one keep-alive apart, so ~10-20 s) means the link is genuinely
// down, not a one-off glitch.
const ERG_FAILURE_THRESHOLD: u32 = 2;

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
                            BleCommand::Scan { reply } => handle_scan(reply, &self.adapter, &self.scan_lock).await,
                            BleCommand::ConnectTrainer { device_id, reply } => self.handle_connect_trainer(device_id, reply).await,
                            BleCommand::ConnectHrm { device_id, reply } => self.handle_connect_hrm(device_id, reply).await,
                            BleCommand::SetTargetPower { watts } => {
                                self.last_target_w = Some(watts);
                                self.send_erg_tracked(watts, "set_target_power").await;
                            }
                            BleCommand::RetryReconnect { kind } => self.handle_retry_reconnect(kind),
                            BleCommand::SessionEnded => {
                                // Session is over: drop the ERG target so the keep-alive
                                // cannot replay it onto a trainer that reconnects later.
                                self.last_target_w = None;
                            }
                        }
                    }
                }
                _keep_alive = keep_alive.tick() => {
                    if let Some(watts) = self.last_target_w {
                        self.send_erg_tracked(watts, "keep-alive").await;
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
                    self.handle_adapter_event(event).await;
                }
                Some(msg) = self.reconnect_rx.recv() => {
                    self.handle_reconnect_msg(msg).await;
                }
            }
        }
    }

    async fn handle_adapter_event(&mut self, event: CentralEvent) {
        let CentralEvent::DeviceDisconnected(id) = event else {
            return;
        };

        if self.trainer.as_ref().map(|p| p.id() == id).unwrap_or(false) {
            // WinRT can emit several DeviceDisconnected events for one drop, including
            // a stale one after we have already reconnected. If the peripheral still
            // reports connected, this is a duplicate/late event, ignore it (issue 18).
            if let Some(trainer) = &self.trainer {
                if trainer.is_connected().await.unwrap_or(false) {
                    info!("ignoring stale DeviceDisconnected for still-connected trainer");
                    return;
                }
            }
            info!("trainer disconnected (adapter event)");
            self.handle_trainer_lost().await;
        } else if self.hrm.as_ref().map(|p| p.id() == id).unwrap_or(false) {
            if let Some(hrm) = &self.hrm {
                if hrm.is_connected().await.unwrap_or(false) {
                    info!("ignoring stale DeviceDisconnected for still-connected hrm");
                    return;
                }
            }
            info!("hrm disconnected");
            self.hrm = None;
            self.last_hr_bpm = None;
            let _ = self.app_handle.emit("ble_disconnected", "hrm");
            // HRM loss never touches session state, so no BleEvent is sent.
            self.start_reconnect(Hrm);
        }
    }

    // Tear down a lost trainer and kick off reconnection. Idempotent: a no-op once
    // the trainer is already gone, so the adapter event and a proactive ERG-failure
    // detection cannot double-trigger.
    async fn handle_trainer_lost(&mut self) {
        if self.trainer.is_none() {
            return;
        }
        self.trainer = None;
        self.last_power_w = None;
        self.last_cadence_rpm = None;
        // Reset so a transient failure right after reconnect does not re-trip the cap.
        self.consecutive_erg_failures = 0;
        // last_target_w is intentionally kept so it can be replayed on reconnect;
        // it is cleared only when the session ends (BleCommand::SessionEnded).
        let _ = self.app_handle.emit("ble_disconnected", "trainer");
        // Critical: the session must pause. send().await (never dropped).
        let _ = self.ble_event_tx.send(BleEvent::TrainerLost).await;
        self.start_reconnect(Trainer);
    }

    // Send an ERG target to the trainer if one is connected, tracking the write
    // outcome so a dropped trainer is detected from consecutive failures. No-op when
    // no trainer is connected.
    async fn send_erg_tracked(&mut self, watts: i16, label: &str) {
        let Some(trainer) = &self.trainer else {
            return;
        };
        let res = send_erg(trainer, watts).await;
        self.note_erg_result(res, label).await;
    }

    // Record the outcome of an ERG write and detect a dropped trainer from it. On
    // WinRT both DeviceDisconnected and is_connected() lag ~40 s, so the write errors
    // themselves are the only timely signal: after ERG_FAILURE_THRESHOLD consecutive
    // failures the trainer is declared lost. A single glitch is tolerated (the count
    // resets on the next success), but a genuinely gone trainer is caught in seconds
    // instead of waiting for the adapter event.
    async fn note_erg_result(&mut self, res: Result<(), AppError>, label: &str) {
        match res {
            Ok(()) => self.consecutive_erg_failures = 0,
            Err(e) => {
                self.consecutive_erg_failures += 1;
                error!(
                    "ERG {label} failed ({}/{ERG_FAILURE_THRESHOLD}): {e}",
                    self.consecutive_erg_failures
                );
                if self.consecutive_erg_failures >= ERG_FAILURE_THRESHOLD {
                    info!("trainer link down (detected via ERG write failures)");
                    self.handle_trainer_lost().await;
                }
            }
        }
    }

    // Manual retry from the UI: relaunch a reconnect task for the retained id.
    fn handle_retry_reconnect(&mut self, kind: DeviceKind) {
        self.start_reconnect(kind);
    }

    // Spawn one reconnect task for `kind`, unless one is already running (which also
    // guards against duplicate disconnect events) or no device id is retained.
    fn start_reconnect(&mut self, kind: DeviceKind) {
        let already_running = match kind {
            Trainer => self.trainer_reconnect_task.is_some(),
            Hrm => self.hrm_reconnect_task.is_some(),
        };
        if already_running {
            info!("reconnect already in progress for {}", kind.as_str());
            return;
        }
        let device_id = match kind {
            Trainer => self.last_trainer_id.clone(),
            Hrm => self.last_hrm_id.clone(),
        };
        let Some(device_id) = device_id else {
            info!("no retained id for {}, cannot reconnect", kind.as_str());
            return;
        };
        let task = tokio::spawn(reconnect_loop(
            self.adapter.clone(),
            device_id,
            kind,
            self.app_handle.clone(),
            self.scan_lock.clone(),
            self.reconnect_tx.clone(),
        ));
        match kind {
            Trainer => self.trainer_reconnect_task = Some(task.abort_handle()),
            Hrm => self.hrm_reconnect_task = Some(task.abort_handle()),
        }
    }

    async fn handle_reconnect_msg(&mut self, msg: ReconnectMsg) {
        match msg {
            ReconnectMsg::Reachable { kind, device_id } => match kind {
                Trainer => {
                    // Finalize through the existing path (abort old notif task,
                    // connect, discover, request control, subscribe, spawn notif task).
                    match self.do_connect_trainer(device_id.clone()).await {
                        Ok(()) => {
                            self.trainer_reconnect_task = None;
                            emit_reconnect(&self.app_handle, Trainer, "reconnected", None);
                            // Critical: tell the session it can auto-resume.
                            let _ = self.ble_event_tx.send(BleEvent::TrainerReconnected).await;
                        }
                        Err(e) => {
                            // Reachable but the connect failed: keep trying.
                            error!("trainer reconnect finalize failed: {e}");
                            self.trainer_reconnect_task = None;
                            self.start_reconnect(Trainer);
                        }
                    }
                }
                Hrm => match self.do_connect_hrm(device_id.clone()).await {
                    Ok(()) => {
                        self.hrm_reconnect_task = None;
                        emit_reconnect(&self.app_handle, Hrm, "reconnected", None);
                    }
                    Err(e) => {
                        error!("hrm reconnect finalize failed: {e}");
                        self.hrm_reconnect_task = None;
                        self.start_reconnect(Hrm);
                    }
                },
            },
            ReconnectMsg::Failed { kind } => {
                // The task already emitted ble_reconnect { failed } and stopped.
                match kind {
                    Trainer => self.trainer_reconnect_task = None,
                    Hrm => self.hrm_reconnect_task = None,
                }
            }
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
        // Retain the id up front so a manual retry can relaunch reconnection even if
        // this attempt fails partway through.
        self.last_trainer_id = Some(device_id.clone());
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
        self.last_hrm_id = Some(device_id.clone());
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
        let _ = self.metrics_tx.try_send(ble_metric);
    }
}

async fn handle_scan(
    reply: Sender<Result<Vec<DeviceInfo>, AppError>>,
    adapter: &Adapter,
    scan_lock: &Arc<Mutex<()>>,
) {
    // Hold the scan lock for the whole scan so a concurrent reconnect-task probe
    // cannot stop_scan() out from under us (issue 18).
    let _guard = scan_lock.lock().await;
    let result = do_scan(adapter).await;
    if let Err(ref e) = result {
        error!("scan failed: {e}");
    }
    let _ = reply.send(result);
}

// Emit the structured ble_reconnect event. `attempt` is only meaningful for the
// "reconnecting" status.
fn emit_reconnect(app_handle: &AppHandle, kind: DeviceKind, status: &str, attempt: Option<u32>) {
    let _ = app_handle.emit(
        "ble_reconnect",
        BleReconnect {
            device: kind.as_str().to_string(),
            status: status.to_string(),
            attempt,
        },
    );
}

// One reconnect task per dropped device. Owns a cloned Adapter + the device id and
// loops at a fixed cadence: re-scan → look for the device → report it reachable back
// to the actor (which owns the actual connect). The actor's select! loop stays
// responsive throughout because this never touches `&mut self`. Stops after a safety
// cap and reports failure (issue 17).
async fn reconnect_loop(
    adapter: Adapter,
    device_id: String,
    kind: DeviceKind,
    app_handle: AppHandle,
    scan_lock: Arc<Mutex<()>>,
    reconnect_tx: MpscSender<ReconnectMsg>,
) {
    let mut ticker = tokio::time::interval(Duration::from_secs(RECONNECT_INTERVAL_S));
    for attempt in 1..=RECONNECT_MAX_ATTEMPTS {
        ticker.tick().await;
        emit_reconnect(&app_handle, kind, "reconnecting", Some(attempt));
        // Bound each attempt so a stalled WinRT scan does not blow past the cadence.
        let reachable = match tokio::time::timeout(
            Duration::from_secs(RECONNECT_ATTEMPT_TIMEOUT_S),
            probe_device(&adapter, &scan_lock, &device_id),
        )
        .await
        {
            Ok(Ok(found)) => found,
            Ok(Err(e)) => {
                error!("reconnect probe error for {}: {e}", kind.as_str());
                false
            }
            Err(_) => {
                info!("reconnect probe timed out for {}", kind.as_str());
                false
            }
        };
        if reachable {
            info!(
                "{} reachable again after {attempt} attempt(s)",
                kind.as_str()
            );
            let _ = reconnect_tx
                .send(ReconnectMsg::Reachable { kind, device_id })
                .await;
            return;
        }
    }
    info!(
        "giving up reconnecting {} after {RECONNECT_MAX_ATTEMPTS} attempts",
        kind.as_str()
    );
    emit_reconnect(&app_handle, kind, "failed", None);
    let _ = reconnect_tx.send(ReconnectMsg::Failed { kind }).await;
}

// Re-scan and report whether the device is present. start_scan/stop_scan is required:
// peripherals() alone does not resurface a sleeping device on WinRT (as do_scan
// proves). Scanning is serialized through the shared lock (issue 18).
async fn probe_device(
    adapter: &Adapter,
    scan_lock: &Arc<Mutex<()>>,
    device_id: &str,
) -> Result<bool, AppError> {
    let _guard = scan_lock.lock().await;
    let _ = adapter.stop_scan().await;
    adapter
        .start_scan(ScanFilter::default())
        .await
        .map_err(|e| AppError::BLEScanError(e.to_string()))?;
    tokio::time::sleep(Duration::from_millis(RECONNECT_SCAN_DWELL_MS)).await;
    adapter
        .stop_scan()
        .await
        .map_err(|e| AppError::BLEScanError(e.to_string()))?;
    let peripherals = adapter
        .peripherals()
        .await
        .map_err(|e| AppError::BLEScanError(e.to_string()))?;
    Ok(peripherals
        .into_iter()
        .any(|p| p.id().to_string() == device_id))
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

    let payload = build_set_target_power_command(watts);
    trainer
        .write(&control_point, &payload, WriteType::WithResponse)
        .await
        .map_err(|e| AppError::BLECommandError(e.to_string()))
}
