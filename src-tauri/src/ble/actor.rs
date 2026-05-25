use btleplug::api::{Central, Peripheral, ScanFilter};
use btleplug::platform;
use btleplug::platform::{Adapter, Peripheral as OtherPeripheral};
use tokio::sync::oneshot::Sender;
use crate::ble::types::{BleActor, BleCommand, DeviceInfo, DeviceKind};
use crate::errors::AppError;
use uuid::Uuid;
use crate::ble::ftms::IndoorBikeData;

// UUIDs standard Bluetooth SIG
const FITNESS_MACHINE_SERVICE: Uuid = Uuid::from_u128(0x00001826_0000_1000_8000_00805f9b34fb);
const HEART_RATE_SERVICE: Uuid = Uuid::from_u128(0x0000180d_0000_1000_8000_00805f9b34fb);
const INDOOR_BIKE_DATA: Uuid = Uuid::from_u128(0x00002AD2_0000_1000_8000_00805f9b34fb);
const HEART_RATE_MEAS: Uuid = Uuid::from_u128(0x00002A37_0000_1000_8000_00805f9b34fb);
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
                        Some(cmd) => if let BleCommand::Scan { reply } = cmd { handle_scan(reply, &self.adapter).await }
                    }
                }
                _keep_alive = keep_alive.tick() => {

                }
                _metrics_ticker = metrics_ticker.tick() => {

                }
            }
        }
    }

    async fn handle_connect_trainer(
        &mut self,
        device_id: String,
        reply: oneshot::Sender<Result<(), AppError>>,
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
        let trainer = self
            .connect_peripheral(device_id, INDOOR_BIKE_DATA)
            .await?;

        self.trainer = Some(trainer);
        Ok(())
    }

    async fn handle_connect_hrm(
        &mut self,
        device_id: String,
        reply: oneshot::Sender<Result<(), AppError>>
    ) {
        let _ = reply.send(self.do_connect_hrm(device_id).await);
    }

    async fn do_connect_hrm(&mut self, device_id: String) -> Result<(), AppError>{
        let hrm = self
            .connect_peripheral(device_id, HEART_RATE_MEAS)
            .await?;
        self.hrm = Some(hrm);
        Ok(())
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

