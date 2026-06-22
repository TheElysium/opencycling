use crate::errors::AppError;
use btleplug::platform::{Adapter, Manager, Peripheral};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{oneshot, Mutex};
use tokio::task::AbortHandle;

#[derive(Serialize, Clone)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub kind: Option<DeviceKind>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Trainer,
    Hrm,
}

impl DeviceKind {
    /// Lowercase wire label used in the `ble_reconnect` / `ble_disconnected` events.
    pub fn as_str(self) -> &'static str {
        match self {
            DeviceKind::Trainer => "trainer",
            DeviceKind::Hrm => "hrm",
        }
    }
}

#[derive(Serialize, Clone)]
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

/// Structured payload for the `ble_reconnect` event. `attempt` is only meaningful
/// while `status == "reconnecting"`.
#[derive(Serialize, Clone)]
pub struct BleReconnect {
    pub device: String,
    pub status: String, // "reconnecting" | "reconnected" | "failed"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
}

/// Critical BLE lifecycle events forwarded to the SessionActor (send().await, never
/// dropped). Only the trainer participates: losing the HRM has no bearing on session
/// state.
#[derive(Debug, Clone, Copy)]
pub enum BleEvent {
    TrainerLost,
    TrainerReconnected,
}

/// Internal message: a reconnect task → the actor. The task only detects that the
/// device is reachable again; the actor owns the actual (re)connect so the tested
/// `do_connect_*` path is reused and there is no notification-loss window.
pub enum ReconnectMsg {
    Reachable { kind: DeviceKind, device_id: String },
    Failed { kind: DeviceKind },
}

// Commands sent from Tauri command handlers → BleActor over the cmd channel.
// Variants with `reply` use a oneshot channel for request-reply: the caller awaits
// the oneshot receiver while the actor processes the command, then sends back the result.
// SetTargetPower has no reply, fire-and-forget is enough for ERG writes.
pub enum BleCommand {
    Scan {
        reply: oneshot::Sender<Result<Vec<DeviceInfo>, AppError>>,
    },
    ConnectTrainer {
        device_id: String,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    ConnectHrm {
        device_id: String,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    SetTargetPower {
        watts: i16,
    },
    /// Manual reconnect requested from the UI (issue 17). The device id is not known
    /// to the frontend, so the actor relaunches a reconnect task for the retained
    /// `last_trainer_id` / `last_hrm_id`.
    RetryReconnect {
        kind: DeviceKind,
    },
    /// The session ended (Stop/Finished). Clears `last_target_w` so the ERG keep-alive
    /// cannot push a stale target onto a trainer that reconnects after the session is
    /// over (issue 17).
    SessionEnded,
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
    pub trainer_task: Option<AbortHandle>,
    pub hrm_task: Option<AbortHandle>,
    pub last_target_w: Option<i16>, // ERG command sent to trainer (outgoing)
    // Consecutive failed ERG writes. WinRT surfaces a dropped trainer through write
    // errors long before DeviceDisconnected (and is_connected() also lags, staying true
    // ~40 s), so repeated failures are the only timely signal. After a small threshold
    // the trainer is declared lost. Reset to 0 on any successful write.
    pub consecutive_erg_failures: u32,
    pub last_power_w: Option<i16>, // actual power measured by trainer (incoming notification)
    pub last_cadence_rpm: Option<u16>,
    pub last_hr_bpm: Option<u16>,
    pub metrics_tx: Sender<BleMetrics>, // fan-out to SessionActor every second
    // Critical lifecycle events (trainer lost/reconnected) → SessionActor.
    pub ble_event_tx: Sender<BleEvent>,
    // Last connected device ids, retained so a manual retry can relaunch a reconnect
    // without the frontend knowing the id (issue 17).
    pub last_trainer_id: Option<String>,
    pub last_hrm_id: Option<String>,
    // Abort handles for the running per-device reconnect tasks. `Some` doubles as the
    // "reconnect in progress" guard against duplicate DeviceDisconnected events (18).
    pub trainer_reconnect_task: Option<AbortHandle>,
    pub hrm_reconnect_task: Option<AbortHandle>,
    // Reconnect tasks report reachability back through this channel.
    pub reconnect_tx: Sender<ReconnectMsg>,
    pub reconnect_rx: Receiver<ReconnectMsg>,
    // Serializes scanning between the UI-triggered scan() and the reconnect tasks so
    // one's stop_scan cannot cut the other's scan (issue 18).
    pub scan_lock: Arc<Mutex<()>>,
}

// Internal message type used on the notif channel (spawned task → actor).
// Values are already parsed; the actor just writes them into its last_* fields.
pub enum ParsedNotifications {
    TrainerData {
        power_w: Option<i16>,
        cadence_rpm: Option<u16>,
    },
    HRMData {
        hr_bpm: u16,
    },
    ParseError {
        device_kind: DeviceKind,
        error: AppError,
    },
}
