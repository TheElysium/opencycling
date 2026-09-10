use crate::ble::sim::{SimActor, SimCommand};
use crate::ble::types::{
    BleActor, BleCommand, BleEvent, BleMetrics, DeviceInfo, DeviceKind, ParsedNotifications,
    ReconnectMsg,
};
use crate::errors::AppError;
use btleplug::api::Manager as _;
use btleplug::platform::Manager;
use std::sync::Arc;
use tauri::{AppHandle, Runtime};
use tokio::spawn;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::sync::{Mutex, oneshot};

// Public handle to the BleActor: only exposes the mpsc Sender so callers
// cannot access actor internals. Cheap to clone; safe to share across threads.
#[derive(Clone)]
pub struct BleActorHandle {
    sender: Sender<BleCommand>,
    // Present only in simulation mode: routes SimCommands to the SimActor.
    sim_tx: Option<Sender<SimCommand>>,
}

impl BleActorHandle {
    pub async fn spawn<R: Runtime>(
        app_handle: AppHandle<R>,
        metrics_tx: Sender<BleMetrics>,
        ble_event_tx: Sender<BleEvent>,
        sim: bool,
    ) -> Result<Self, AppError> {
        let (cmd_tx, cmd_rx) = channel::<BleCommand>(32);
        if sim {
            return Ok(Self::spawn_sim(
                app_handle,
                cmd_tx,
                cmd_rx,
                metrics_tx,
                ble_event_tx,
            ));
        }

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
            trainer_control_point: None,
            hrm: None,
            trainer_task: None,
            hrm_task: None,
            last_target_w: None,
            consecutive_erg_failures: 0,
            last_power_w: None,
            last_cadence_rpm: None,
            app_handle,
            last_hr_bpm: None,
            last_trainer_notif: None,
            last_hrm_notif: None,
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

        Ok(Self {
            sender: cmd_tx,
            sim_tx: None,
        })
    }

    // Sim mode: no btleplug Manager/Adapter is created at all; a SimActor consumes the
    // same BleCommand stream and additionally serves the SimCommand channel.
    fn spawn_sim<R: Runtime>(
        app_handle: AppHandle<R>,
        sender: Sender<BleCommand>,
        cmd_rx: Receiver<BleCommand>,
        metrics_tx: Sender<BleMetrics>,
        ble_event_tx: Sender<BleEvent>,
    ) -> Self {
        let (sim_tx, sim_rx) = channel::<SimCommand>(16);
        spawn(SimActor::new(app_handle, cmd_rx, sim_rx, metrics_tx, ble_event_tx).run());
        Self {
            sender,
            sim_tx: Some(sim_tx),
        }
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

    // User-initiated disconnect: tear the device down for good so auto-reconnect does
    // not resurrect it. Request-reply so the caller knows the actor processed it.
    pub async fn disconnect(&self, kind: DeviceKind) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(BleCommand::Disconnect { kind, reply: tx })
            .await
            .map_err(|_| AppError::ChannelClosed)?;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    // Fire-and-forget: clear the retained ERG target when a session ends so the
    // keep-alive cannot resurrect it on a later reconnect (issue 17).
    pub async fn session_ended(&self) -> Result<(), AppError> {
        self.sender
            .send(BleCommand::SessionEnded)
            .await
            .map_err(|_| AppError::ChannelClosed)
    }

    // Simulation-only scenario triggers; error out on a real BLE run.
    pub async fn sim_drop_device(&self, kind: DeviceKind, stay_lost: bool) -> Result<(), AppError> {
        let tx = self
            .sim_tx
            .as_ref()
            .ok_or_else(|| AppError::Other("simulation mode is disabled".to_string()))?;
        tx.send(SimCommand::Drop { kind, stay_lost })
            .await
            .map_err(|_| AppError::ChannelClosed)
    }

    pub async fn sim_restore_device(&self, kind: DeviceKind) -> Result<(), AppError> {
        let tx = self
            .sim_tx
            .as_ref()
            .ok_or_else(|| AppError::Other("simulation mode is disabled".to_string()))?;
        tx.send(SimCommand::Restore { kind })
            .await
            .map_err(|_| AppError::ChannelClosed)
    }

    pub async fn sim_set_pedaling(&self, pedaling: bool) -> Result<(), AppError> {
        let tx = self
            .sim_tx
            .as_ref()
            .ok_or_else(|| AppError::Other("simulation mode is disabled".to_string()))?;
        tx.send(SimCommand::SetPedaling { pedaling })
            .await
            .map_err(|_| AppError::ChannelClosed)
    }
}
