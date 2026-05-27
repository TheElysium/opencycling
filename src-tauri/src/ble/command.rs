use btleplug::platform::Manager;
use btleplug::api::Manager as _;
use tauri::AppHandle;
use tokio::spawn;
use tokio::sync::mpsc::{channel, Sender};
use tokio::sync::oneshot;
use crate::ble::types::{BleActor, BleCommand, DeviceInfo, ParsedNotifications};
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


        let (cmd_tx,cmd_rx) = channel::<BleCommand>(32);
        let (notif_tx,notif_rx) = channel::<ParsedNotifications>(64);
        let ble_actor = BleActor{
            cmd_rx,
            notif_tx,
            notif_rx,
            adapter,
            _manager: manager,
            trainer: None,
            hrm: None,
            last_target_w: None,
            last_power_w: None,
            last_cadence_rpm: None,
            app_handle,
            last_hr_bpm: None,
        };

        spawn(ble_actor.run());

        Ok(Self {sender: cmd_tx})
    }

    pub async fn scan(&self) -> Result<Vec<DeviceInfo>, AppError> {
        let (tx,rx) = oneshot::channel::<Result<Vec<DeviceInfo>, AppError>>();
        let cmd = BleCommand::Scan {reply:tx};
        self.sender.send(cmd)
            .await
            .map_err(|_| AppError::BLEScanError(String::from("Failed to send BLE command")))?;

        rx.await.map_err(|_| AppError::BLEScanError(String::from("Failed to receive BLE command")))?
    }

    pub async fn connect_trainer(&self, device_id: String) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(BleCommand::ConnectTrainer { device_id, reply: tx })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn set_target_power(&self, watts: i16) -> Result<(), AppError> {
        self.sender.send(BleCommand::SetTargetPower { watts })
            .await
            .map_err(|_| AppError::ChannelClosed)
    }

    pub async fn connect_hrm(&self, device_id: String) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(BleCommand::ConnectHrm { device_id, reply: tx })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }
}