use tokio::sync::mpsc::Receiver;
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

#[derive(Serialize)]
pub struct BleError {
    pub device: String,
    pub message: String,
}

pub enum BleCommand {
    Scan      { reply: oneshot::Sender<Result<Vec<DeviceInfo>, AppError>> },
    ConnectTrainer { device_id: String, reply: oneshot::Sender<Result<(), AppError>> },
    ConnectHrm     { device_id: String, reply: oneshot::Sender<Result<(), AppError>> },
    SetTargetPower { watts: i16 },
}

pub struct BleActor {
    pub cmd_rx: Receiver<BleCommand>,
    pub adapter: Adapter,
    pub _manager: Manager,
    pub trainer: Option<Peripheral>,
    pub hrm: Option<Peripheral>,
    pub last_target_w: Option<i16>,
    pub app_handle: AppHandle,
}