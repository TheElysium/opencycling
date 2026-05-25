use btleplug::platform::Manager;
use btleplug::api::Manager as _;
use tauri::AppHandle;
use tokio::spawn;
use tokio::sync::mpsc::{channel, Sender};
use tokio::sync::oneshot;
use crate::ble::types::{BleActor, BleCommand, DeviceInfo};
use crate::errors::AppError;




pub struct BleActorHandle {
    sender: Sender<BleCommand>,
}

impl BleActorHandle {
    pub async fn spawn(app_handle: AppHandle) -> Result<Self, AppError> {
        let manager = Manager::new()
            .await
            .map_err(|err| AppError::BLEScanError(err.to_string()))?;

        let adapters = manager.adapters()
            .await
            .map_err(|err| AppError::BLEScanError(err.to_string()))?;

        let adapter = adapters
            .into_iter()
            .next()
            .ok_or_else(|| AppError::DeviceNotFound("No BLE adapter".to_string()))?;


        let (tx,rx) = channel::<BleCommand>(32);
        let ble_actor = BleActor{
            cmd_rx: rx,
            adapter,
            _manager: manager,
            trainer: None,
            hrm: None,
            last_target_w: None,
            app_handle,
        };

        spawn(ble_actor.run());

        Ok(Self {sender: tx})
    }

    pub async fn scan(&self) -> Result<Vec<DeviceInfo>, AppError> {
        let (tx,rx) = oneshot::channel::<Result<Vec<DeviceInfo>, AppError>>();
        let cmd = BleCommand::Scan {reply:tx};
        self.sender.send(cmd)
            .await
            .map_err(|_| AppError::BLEScanError(String::from("Failed to send BLE command")))?;

        rx.await.map_err(|_| AppError::BLEScanError(String::from("Failed to receive BLE command")))?
    }
}