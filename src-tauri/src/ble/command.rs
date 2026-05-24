use btleplug::platform::{Adapter, Manager};
use btleplug::api::Manager as _;
use tokio::spawn;
use tokio::sync::mpsc::{channel, Sender};
use crate::ble::types::{BleActor, BleCommand};
use crate::errors::AppError;

pub struct BleActorHandle {
    sender: Sender<BleCommand>,
}

impl BleActorHandle {
    pub async fn spawn() -> Result<Self, AppError> {
        let manager = Manager::new()
            .await
            .map_err(|err| AppError::BLEScanError(err.to_string()))?;

        let adapters = manager.adapters()
            .await
            .map_err(|err| AppError::BLEScanError(err.to_string()))?;

        let adapter = adapters
            .into_iter()
            .nth(0)
            .ok_or_else(|| AppError::DeviceNotFound("No BLE adapter".to_string()))?;


        let (tx,rx) = channel::<BleCommand>(32);
        let ble_actor = BleActor{
            cmd_rx: rx,
            adapter,
            _manager: manager,
        };

        spawn(ble_actor.run());

        Ok(Self {sender: tx})
    }
}