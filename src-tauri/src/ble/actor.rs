use btleplug::api::{Central, Peripheral, ScanFilter};
use btleplug::platform::Adapter;
use tokio::sync::oneshot::Sender;
use crate::ble::types::{BleActor, BleCommand, DeviceInfo, DeviceKind};
use crate::errors::AppError;
use uuid::Uuid;


// UUIDs standard Bluetooth SIG
const FITNESS_MACHINE_SERVICE: Uuid = Uuid::from_u128(0x00001826_0000_1000_8000_00805f9b34fb);
const HEART_RATE_SERVICE: Uuid = Uuid::from_u128(0x0000180d_0000_1000_8000_00805f9b34fb);


impl BleActor {
    pub async fn run(mut self) {
        loop {
            match self.cmd_rx.recv().await {
                None => {break}
                Some(cmd) => if let BleCommand::Scan { reply } = cmd { handle_scan(reply, &self.adapter).await }
            }
        }
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