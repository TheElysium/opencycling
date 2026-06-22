use crate::ble::types::{
    BleActor, BleCommand, BleEvent, BleMetrics, DeviceInfo, DeviceKind, ParsedNotifications,
    ReconnectMsg,
};
use crate::errors::AppError;
use btleplug::api::Manager as _;
use btleplug::platform::Manager;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::spawn;
use tokio::sync::mpsc::{channel, Sender};
use tokio::sync::{oneshot, Mutex};

// Public handle to the BleActor: only exposes the mpsc Sender so callers
// cannot access actor internals. Cheap to clone; safe to share across threads.
#[derive(Clone)]
pub struct BleActorHandle {
    sender: Sender<BleCommand>,
}

impl BleActorHandle {
    pub async fn spawn(
        app_handle: AppHandle,
        metrics_tx: Sender<BleMetrics>,
        ble_event_tx: Sender<BleEvent>,
    ) -> Result<Self, AppError> {
        let manager = Manager::new()
            .await
            .map_err(|err| AppError::BLEScanError(err.to_string()))?;

        let adapters = manager
            .adapters()
            .await
            .map_err(|err| AppError::BLEScanError(err.to_string()))?;

        let adapter = adapters
            .into_iter()
            .next()
            .ok_or_else(|| AppError::DeviceNotFound("No BLE adapter".to_string()))?;

        // cmd channel: Tauri handlers → actor (commands and replies).
        let (cmd_tx, cmd_rx) = channel::<BleCommand>(32);
        // notif channel: per-device spawned tasks → actor (parsed BLE notifications).
        let (notif_tx, notif_rx) = channel::<ParsedNotifications>(64);
        // reconnect channel: reconnect tasks → actor (device reachable / gave up).
        let (reconnect_tx, reconnect_rx) = channel::<ReconnectMsg>(16);
        let ble_actor = BleActor {
            cmd_rx,
            notif_tx,
            notif_rx,
            adapter,
            _manager: manager,
            trainer: None,
            hrm: None,
            trainer_task: None,
            hrm_task: None,
            last_target_w: None,
            consecutive_erg_failures: 0,
            last_power_w: None,
            last_cadence_rpm: None,
            app_handle,
            last_hr_bpm: None,
            metrics_tx,
            ble_event_tx,
            last_trainer_id: None,
            last_hrm_id: None,
            trainer_reconnect_task: None,
            hrm_reconnect_task: None,
            reconnect_tx,
            reconnect_rx,
            scan_lock: Arc::new(Mutex::new(())),
        };

        spawn(ble_actor.run());

        Ok(Self { sender: cmd_tx })
    }

    pub async fn scan(&self) -> Result<Vec<DeviceInfo>, AppError> {
        // Request-reply over two channels: send the command with a oneshot tx,
        // then await the rx. The actor sends the result back through that oneshot.
        let (tx, rx) = oneshot::channel::<Result<Vec<DeviceInfo>, AppError>>();
        let cmd = BleCommand::Scan { reply: tx };
        self.sender
            .send(cmd)
            .await
            .map_err(|_| AppError::BLEScanError(String::from("Failed to send BLE command")))?;

        rx.await
            .map_err(|_| AppError::BLEScanError(String::from("Failed to receive BLE command")))?
    }

    pub async fn connect_trainer(&self, device_id: String) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(BleCommand::ConnectTrainer {
                device_id,
                reply: tx,
            })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn set_target_power(&self, watts: i16) -> Result<(), AppError> {
        // Fire-and-forget: no oneshot reply needed; the actor stores watts and the
        // keep-alive interval retransmits it every 10 s.
        self.sender
            .send(BleCommand::SetTargetPower { watts })
            .await
            .map_err(|_| AppError::ChannelClosed)
    }

    pub async fn connect_hrm(&self, device_id: String) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(BleCommand::ConnectHrm {
                device_id,
                reply: tx,
            })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    // Fire-and-forget: relaunch a reconnect task for the given device kind. The actor
    // uses the retained device id; if none is known or a reconnect is already running,
    // the actor ignores it.
    pub async fn retry_reconnect(&self, kind: DeviceKind) -> Result<(), AppError> {
        self.sender
            .send(BleCommand::RetryReconnect { kind })
            .await
            .map_err(|_| AppError::ChannelClosed)
    }

    // Fire-and-forget: clear the retained ERG target when a session ends so the
    // keep-alive cannot resurrect it on a later reconnect (issue 17).
    pub async fn session_ended(&self) -> Result<(), AppError> {
        self.sender
            .send(BleCommand::SessionEnded)
            .await
            .map_err(|_| AppError::ChannelClosed)
    }
}
