use tokio::sync::mpsc::{Receiver, Sender};
use btleplug::platform::{Adapter, Manager, Peripheral};
use serde::Serialize;
use tauri::AppHandle;
use tokio::sync::oneshot;
use crate::errors::AppError;

#[derive(Serialize, Clone)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub kind: Option<DeviceKind>,
}

#[derive(Serialize, Clone)]
pub enum DeviceKind {
    Trainer,
    Hrm,
}

#[derive(Serialize)]
pub struct BleMetrics {
    pub power_w: Option<i16>,
    pub hr_bpm: Option<u16>,
    pub cadence_rpm: Option<u16>,
}

#[derive(Serialize, Clone)]
pub struct BleError {
    pub device: String,
    pub message: String,
}

// Commands sent from Tauri command handlers → BleActor over the cmd channel.
// Variants with `reply` use a oneshot channel for request-reply: the caller awaits
// the oneshot receiver while the actor processes the command, then sends back the result.
// SetTargetPower has no reply — fire-and-forget is enough for ERG writes.
pub enum BleCommand {
    Scan      { reply: oneshot::Sender<Result<Vec<DeviceInfo>, AppError>> },
    ConnectTrainer { device_id: String, reply: oneshot::Sender<Result<(), AppError>> },
    ConnectHrm     { device_id: String, reply: oneshot::Sender<Result<(), AppError>> },
    SetTargetPower { watts: i16 },
}

pub struct BleActor {
    pub app_handle: AppHandle,
    // Receives BleCommands from BleActorHandle (Tauri command handlers → actor).
    pub cmd_rx: Receiver<BleCommand>,
    // Per-device notification tasks clone notif_tx and forward parsed BLE values.
    // The actor holds notif_rx and drains it in the select! loop.
    // Two channels are needed because btleplug streams cannot be polled directly
    // inside select! while &mut self is borrowed elsewhere.
    pub notif_tx: Sender<ParsedNotifications>,
    pub notif_rx: Receiver<ParsedNotifications>,
    pub adapter: Adapter,
    pub _manager: Manager,
    pub trainer: Option<Peripheral>,
    pub hrm: Option<Peripheral>,
    pub last_target_w: Option<i16>,  // ERG command sent to trainer (outgoing)
    pub last_power_w: Option<i16>,   // actual power measured by trainer (incoming notification)
    pub last_cadence_rpm: Option<u16>,
    pub last_hr_bpm: Option<u16>,
}

// Internal message type used on the notif channel (spawned task → actor).
// Values are already parsed; the actor just writes them into its last_* fields.
pub enum ParsedNotifications {
    TrainerData{power_w: Option<i16>, cadence_rpm: Option<u16>},
    HRMData{hr_bpm: u16},
    ParseError{device_kind: DeviceKind,error: AppError},
}