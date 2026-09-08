use crate::ble::actor::emit_reconnect;
use crate::ble::types::{BleCommand, BleEvent, BleMetrics, DeviceInfo, DeviceKind};
use crate::errors::AppError;
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::oneshot::Sender as ReplySender;
use tokio::task::AbortHandle;
use tracing::info;
use DeviceKind::{Hrm, Trainer};

// Sim metrics cadence mirrors actor.rs METRICS_TICK (1 Hz emit_metrics).
const SIM_METRICS_TICK_S: u64 = 1;
// Reconnect cadence + cap mirror actor.rs RECONNECT_INTERVAL_S / RECONNECT_MAX_ATTEMPTS
// so the simulator reproduces the real ~30 s give-up window.
pub const SIM_RECONNECT_INTERVAL_S: u64 = 3;
pub const SIM_RECONNECT_MAX_ATTEMPTS: u32 = 10;
// Without stay_lost the dropped device "comes back" after this many attempts, giving
// the frontend a few reconnecting events to render before the auto-recovery.
pub const SIM_RECOVER_AFTER_ATTEMPTS: u32 = 4;
// First-order power ramp gain per 1 s tick: tau ≈ 1 s, so the ramp settles in ~3 s (3τ).
const SIM_RAMP_PER_TICK: f64 = 0.63;
// HR reacts slower than power (physiological lag), tau ≈ 3 s.
const SIM_HR_LAG_PER_TICK: f64 = 0.3;
const SIM_HR_REST_BPM: u16 = 70;
const SIM_HR_WORK_BASE_BPM: u16 = 95;
const SIM_TRAINER_ID: &str = "sim-trainer";
const SIM_HRM_ID: &str = "sim-hrm";
const SIM_CADENCE_RPM: i16 = 88;

static SIM_ENABLED: OnceLock<bool> = OnceLock::new();

// Called once from lib.rs::run before any command can query it; set() failing on a
// second call cannot happen at startup.
pub fn set_enabled(enabled: bool) {
    let _ = SIM_ENABLED.set(enabled);
}

pub fn is_enabled() -> bool {
    *SIM_ENABLED.get().unwrap_or(&false)
}

// OPENYCLING_SIM=1/true (case-insensitive) swaps the whole BLE stack for the simulator.
pub fn enabled_from_env() -> bool {
    match std::env::var("OPENYCLING_SIM") {
        Ok(v) => v.eq_ignore_ascii_case("1") || v.eq_ignore_ascii_case("true"),
        Err(_) => false,
    }
}

// Sim-only commands arrive on their own channel so BleCommand stays unpolluted.
pub enum SimCommand {
    Drop { kind: DeviceKind, stay_lost: bool },
    Restore { kind: DeviceKind },
}

enum SimReconnectMsg {
    Tick { kind: DeviceKind },
}

/// Per-device scenario, the pure heart of the sim: transitions are unit-testable
/// without tokio (Connected / Reconnecting { attempt, stay_lost } / Failed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SimDeviceState {
    Connected,
    Reconnecting { attempt: u32, stay_lost: bool },
    Failed,
}

/// One reconnect attempt resolved by the scenario state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SimAttempt {
    Continue(u32),
    Recovered(u32),
    Failed(u32),
}

#[derive(Debug, Default)]
struct SimDevice {
    // Retained device id: Some from first connect until a user disconnect (mirrors
    // actor.rs last_trainer_id / last_hrm_id semantics).
    id: Option<String>,
    state: Option<SimDeviceState>,
    // Plugged back in (sim_restore_device): the next reconnect attempt succeeds.
    present: bool,
}

impl SimDevice {
    fn connected(&self) -> bool {
        matches!(self.state, Some(SimDeviceState::Connected))
    }

    fn reconnecting(&self) -> bool {
        matches!(self.state, Some(SimDeviceState::Reconnecting { .. }))
    }

    // Mirror of actor.rs do_connect_trainer / do_connect_hrm id retention.
    fn connect(&mut self, id: String) {
        self.id = Some(id);
        self.state = Some(SimDeviceState::Connected);
    }

    // Start a drop scenario. No-op (false) unless currently connected.
    fn drop_device(&mut self, stay_lost: bool) -> bool {
        if !self.connected() {
            return false;
        }
        self.present = false;
        self.state = Some(SimDeviceState::Reconnecting {
            attempt: 0,
            stay_lost,
        });
        true
    }

    // User disconnect clears the retained id so no retry can relaunch (mirror of
    // actor.rs BleActor::handle_disconnect).
    fn disconnect_user(&mut self) {
        self.id = None;
        self.state = None;
        self.present = false;
    }

    fn restore(&mut self) {
        self.present = true;
    }

    // Manual retry: a fresh loop that recovers as soon as the device shows up; a
    // manual retry implies the user fixed the link, so stay_lost is not carried over.
    fn retry(&mut self) -> bool {
        if self.id.is_none() || self.reconnecting() {
            return false;
        }
        self.state = Some(SimDeviceState::Reconnecting {
            attempt: 0,
            stay_lost: false,
        });
        true
    }

    // One reconnect attempt: the loop reports it, the actor resolves it. Recovery
    // wins over the cap so a restored device comes back even in a stay_lost scenario.
    fn advance(&mut self) -> Option<SimAttempt> {
        let Some(SimDeviceState::Reconnecting { attempt, stay_lost }) = self.state else {
            return None;
        };
        let attempt = attempt + 1;
        let recovered = self.present || (!stay_lost && attempt >= SIM_RECOVER_AFTER_ATTEMPTS);
        if recovered {
            self.present = false;
            self.state = Some(SimDeviceState::Connected);
            return Some(SimAttempt::Recovered(attempt));
        }
        if attempt >= SIM_RECONNECT_MAX_ATTEMPTS {
            self.state = Some(SimDeviceState::Failed);
            return Some(SimAttempt::Failed(attempt));
        }
        self.state = Some(SimDeviceState::Reconnecting { attempt, stay_lost });
        Some(SimAttempt::Continue(attempt))
    }
}

pub(crate) struct SimActor<R: Runtime> {
    app_handle: AppHandle<R>,
    cmd_rx: Receiver<BleCommand>,
    sim_rx: Receiver<SimCommand>,
    reconnect_tx: Sender<SimReconnectMsg>,
    reconnect_rx: Receiver<SimReconnectMsg>,
    metrics_tx: Sender<BleMetrics>,
    ble_event_tx: Sender<BleEvent>,
    trainer: SimDevice,
    hrm: SimDevice,
    trainer_reconnect_task: Option<AbortHandle>,
    hrm_reconnect_task: Option<AbortHandle>,
    last_target_w: Option<i16>,
    power_w: i16,
    hr_bpm: u16,
    // Deterministic noise phase, advanced once per emitted metrics tick.
    tick: u64,
}

impl<R: Runtime> SimActor<R> {
    pub(crate) fn new(
        app_handle: AppHandle<R>,
        cmd_rx: Receiver<BleCommand>,
        sim_rx: Receiver<SimCommand>,
        metrics_tx: Sender<BleMetrics>,
        ble_event_tx: Sender<BleEvent>,
    ) -> Self {
        let (reconnect_tx, reconnect_rx) = channel::<SimReconnectMsg>(16);
        Self {
            app_handle,
            cmd_rx,
            sim_rx,
            reconnect_tx,
            reconnect_rx,
            metrics_tx,
            ble_event_tx,
            trainer: SimDevice::default(),
            hrm: SimDevice::default(),
            trainer_reconnect_task: None,
            hrm_reconnect_task: None,
            last_target_w: None,
            power_w: 0,
            hr_bpm: SIM_HR_REST_BPM,
            tick: 0,
        }
    }

    pub(crate) async fn run(mut self) {
        info!("simulation mode active: no BLE hardware will be used");
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(SIM_METRICS_TICK_S));
        loop {
            tokio::select! {
                cmd = self.cmd_rx.recv() => match cmd {
                    None => break,
                    Some(cmd) => self.handle_ble_command(cmd).await,
                },
                cmd = self.sim_rx.recv() => match cmd {
                    None => break,
                    Some(cmd) => self.handle_sim_command(cmd).await,
                },
                Some(msg) = self.reconnect_rx.recv() => self.handle_reconnect_tick(msg).await,
                _ = ticker.tick() => self.emit_metrics(),
            }
        }
    }

    // Mirror of the BleCommand arms of BleActor::run (ble/actor.rs), with hardware
    // side effects replaced by plain state updates.
    async fn handle_ble_command(&mut self, cmd: BleCommand) {
        match cmd {
            BleCommand::Scan { reply } => {
                let _ = reply.send(Ok(sim_scan_results()));
            }
            BleCommand::ConnectTrainer { device_id, reply } => {
                self.trainer.connect(device_id);
                self.power_w = 0;
                let _ = reply.send(Ok(()));
            }
            BleCommand::ConnectHrm { device_id, reply } => {
                self.hrm.connect(device_id);
                self.hr_bpm = SIM_HR_REST_BPM;
                let _ = reply.send(Ok(()));
            }
            BleCommand::SetTargetPower { watts } => self.last_target_w = Some(watts),
            BleCommand::SessionEnded => self.last_target_w = None,
            BleCommand::RetryReconnect { kind } => self.handle_retry_reconnect(kind),
            BleCommand::Disconnect { kind, reply } => self.handle_disconnect(kind, reply),
        }
    }

    fn handle_disconnect(&mut self, kind: DeviceKind, reply: ReplySender<Result<(), AppError>>) {
        self.stop_reconnect_task(kind);
        match kind {
            Trainer => {
                self.trainer.disconnect_user();
                self.last_target_w = None;
                self.power_w = 0;
            }
            Hrm => self.hrm.disconnect_user(),
        }
        let _ = reply.send(Ok(()));
    }

    // Mirror of actor.rs BleActor::handle_retry_reconnect: relaunch a loop only when
    // a device id is retained and no loop is running for that kind.
    fn handle_retry_reconnect(&mut self, kind: DeviceKind) {
        let started = match kind {
            Trainer => self.trainer.retry(),
            Hrm => self.hrm.retry(),
        };
        if started {
            self.spawn_reconnect_loop(kind);
        }
    }

    async fn handle_sim_command(&mut self, cmd: SimCommand) {
        match cmd {
            SimCommand::Drop { kind, stay_lost } => self.handle_drop(kind, stay_lost).await,
            SimCommand::Restore { kind } => match kind {
                Trainer => self.trainer.restore(),
                Hrm => self.hrm.restore(),
            },
        }
    }

    // Mirror of ble/actor.rs handle_trainer_lost + the HRM branch of
    // handle_adapter_event: teardown, ble_disconnected, TrainerLost for the trainer
    // only, then a reconnect loop.
    async fn handle_drop(&mut self, kind: DeviceKind, stay_lost: bool) {
        let dropped = match kind {
            Trainer => self.trainer.drop_device(stay_lost),
            Hrm => self.hrm.drop_device(stay_lost),
        };
        if !dropped {
            return;
        }
        if kind == Trainer {
            self.power_w = 0;
        }
        self.stop_reconnect_task(kind);
        let _ = self.app_handle.emit("ble_disconnected", kind.as_str());
        if let Some(event) = lost_event(kind) {
            // Critical: the session must pause. send().await, never dropped.
            let _ = self.ble_event_tx.send(event).await;
        }
        self.spawn_reconnect_loop(kind);
    }

    fn spawn_reconnect_loop(&mut self, kind: DeviceKind) {
        let task = tokio::spawn(sim_reconnect_loop(kind, self.reconnect_tx.clone()));
        match kind {
            Trainer => self.trainer_reconnect_task = Some(task.abort_handle()),
            Hrm => self.hrm_reconnect_task = Some(task.abort_handle()),
        }
    }

    fn stop_reconnect_task(&mut self, kind: DeviceKind) {
        let handle = match kind {
            Trainer => self.trainer_reconnect_task.take(),
            Hrm => self.hrm_reconnect_task.take(),
        };
        if let Some(handle) = handle {
            handle.abort();
        }
    }

    // Mirror of ble/actor.rs handle_reconnect_msg: the loop reports an attempt, the
    // actor owns the scenario state machine and emits the same ble_reconnect shapes.
    async fn handle_reconnect_tick(&mut self, msg: SimReconnectMsg) {
        let SimReconnectMsg::Tick { kind } = msg;
        let Some(attempt) = (match kind {
            Trainer => self.trainer.advance(),
            Hrm => self.hrm.advance(),
        }) else {
            return;
        };
        match attempt {
            SimAttempt::Continue(n) => {
                emit_reconnect(&self.app_handle, kind, "reconnecting", Some(n));
            }
            SimAttempt::Recovered(n) => {
                emit_reconnect(&self.app_handle, kind, "reconnecting", Some(n));
                // The loop keeps ticking until aborted; recovery ends it early.
                self.stop_reconnect_task(kind);
                emit_reconnect(&self.app_handle, kind, "reconnected", None);
                if let Some(event) = reconnected_event(kind) {
                    // Critical: the session may auto-resume. send().await, never dropped.
                    let _ = self.ble_event_tx.send(event).await;
                }
            }
            SimAttempt::Failed(n) => {
                emit_reconnect(&self.app_handle, kind, "reconnecting", Some(n));
                emit_reconnect(&self.app_handle, kind, "failed", None);
                self.stop_reconnect_task(kind);
            }
        }
    }

    // Mirror of ble/actor.rs BleActor::emit_metrics: same 1 Hz tick, same emit +
    // metrics_tx.try_send fan-out, gated on any connected device.
    fn emit_metrics(&mut self) {
        if !self.trainer.connected() && !self.hrm.connected() {
            return;
        }
        self.tick += 1;
        let power_w = if self.trainer.connected() {
            self.power_w = sim_step(
                self.power_w,
                self.last_target_w.unwrap_or(0),
                SIM_RAMP_PER_TICK,
            );
            Some((self.power_w + sim_noise(self.tick)).max(0))
        } else {
            None
        };
        let cadence_rpm = self.trainer.connected().then(|| sim_cadence(self.tick));
        let hr_bpm = if self.hrm.connected() {
            let target = sim_target_hr(self.trainer.connected(), self.power_w);
            self.hr_bpm = sim_step(self.hr_bpm as i16, target as i16, SIM_HR_LAG_PER_TICK) as u16;
            Some(self.hr_bpm)
        } else {
            None
        };
        let metrics = BleMetrics {
            power_w,
            hr_bpm,
            cadence_rpm,
        };
        let _ = self.app_handle.emit("ble_metrics", &metrics);
        let _ = self.metrics_tx.try_send(metrics);
    }
}

// One task per dropped device, mirroring ble/actor.rs reconnect_loop: fixed cadence,
// reports each attempt back to the actor, which owns the scenario state machine. The
// actor aborts the task on recovery or user disconnect.
async fn sim_reconnect_loop(kind: DeviceKind, reconnect_tx: Sender<SimReconnectMsg>) {
    for _ in 1..=SIM_RECONNECT_MAX_ATTEMPTS {
        tokio::time::sleep(std::time::Duration::from_secs(SIM_RECONNECT_INTERVAL_S)).await;
        if reconnect_tx
            .send(SimReconnectMsg::Tick { kind })
            .await
            .is_err()
        {
            return;
        }
    }
}

// One first-order step toward target. A minimum step of 1 avoids an integer-rounding
// plateau that would freeze the value just below target; the clamp prevents overshoot.
fn sim_step(current: i16, target: i16, gain: f64) -> i16 {
    let mut next = current + ((target - current) as f64 * gain).round() as i16;
    if next == current && target != current {
        next += (target - current).signum();
    }
    next.clamp(current.min(target), current.max(target))
}

// Deterministic wobble in [-2, 2]: plots look alive without a RNG and tests stay
// reproducible.
fn sim_noise(tick: u64) -> i16 {
    (tick % 5) as i16 - 2
}

fn sim_cadence(tick: u64) -> u16 {
    (SIM_CADENCE_RPM + sim_noise(tick.wrapping_mul(3))) as u16
}

// Mirror of the sim spec: hr chases 95 + power/2 while the trainer pushes watts,
// resting ~70 otherwise.
fn sim_target_hr(trainer_connected: bool, power_w: i16) -> u16 {
    if trainer_connected && power_w > 0 {
        SIM_HR_WORK_BASE_BPM + (power_w / 2) as u16
    } else {
        SIM_HR_REST_BPM
    }
}

// Mirror of ble/actor.rs: only the trainer drives session state via BleEvent; the
// HRM never sends one.
fn lost_event(kind: DeviceKind) -> Option<BleEvent> {
    match kind {
        Trainer => Some(BleEvent::TrainerLost),
        Hrm => None,
    }
}

fn reconnected_event(kind: DeviceKind) -> Option<BleEvent> {
    match kind {
        Trainer => Some(BleEvent::TrainerReconnected),
        Hrm => None,
    }
}

// Mirror of ble/actor.rs do_scan output shape: one trainer and one HRM, both named
// with the prefixes the app filters on ("D500" / "Polar").
fn sim_scan_results() -> Vec<DeviceInfo> {
    vec![
        DeviceInfo {
            id: SIM_TRAINER_ID.to_string(),
            name: "D500 SIM Trainer".to_string(),
            kind: Some(Trainer),
        },
        DeviceInfo {
            id: SIM_HRM_ID.to_string(),
            name: "Polar SIM HRM".to_string(),
            kind: Some(Hrm),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_ramp_converges_in_about_3s_without_overshoot() {
        let mut p = 0;
        for _ in 0..3 {
            p = sim_step(p, 200, SIM_RAMP_PER_TICK);
            assert!(p <= 200);
        }
        // 3τ: ~95 % of the target reached after ~3 s.
        assert!(p >= 190);
        for _ in 0..20 {
            p = sim_step(p, 200, SIM_RAMP_PER_TICK);
        }
        assert_eq!(p, 200);
    }

    #[test]
    fn power_ramp_is_monotone_down_and_never_undershoots() {
        let mut p = 250;
        let mut prev = p;
        for _ in 0..50 {
            p = sim_step(p, 100, SIM_RAMP_PER_TICK);
            assert!(p <= prev && p >= 100);
            prev = p;
        }
        assert_eq!(p, 100);
    }

    #[test]
    fn hr_lag_approaches_target_asymptotically() {
        let target = sim_target_hr(true, 200) as i16;
        assert_eq!(target, 195);
        let mut hr = SIM_HR_REST_BPM as i16;
        let mut prev = hr;
        for _ in 0..30 {
            hr = sim_step(hr, target, SIM_HR_LAG_PER_TICK);
            assert!(hr >= prev && hr <= target);
            prev = hr;
        }
        assert_eq!(hr, target);
    }

    #[test]
    fn sim_target_hr_rests_without_trainer_power() {
        assert_eq!(sim_target_hr(false, 250), SIM_HR_REST_BPM);
        assert_eq!(sim_target_hr(true, 0), SIM_HR_REST_BPM);
        assert_eq!(sim_target_hr(true, 200), SIM_HR_WORK_BASE_BPM + 100);
    }

    #[test]
    fn noise_and_cadence_stay_in_expected_band() {
        for tick in 0..100u64 {
            assert!((-2..=2).contains(&sim_noise(tick)));
            assert!((86..=90).contains(&sim_cadence(tick)));
        }
    }

    fn connected_device(id: &str) -> SimDevice {
        let mut device = SimDevice::default();
        device.connect(id.to_string());
        device
    }

    #[test]
    fn drop_auto_recovers_at_attempt_4() {
        let mut device = connected_device("sim-trainer");
        assert!(device.drop_device(false));
        assert_eq!(device.advance(), Some(SimAttempt::Continue(1)));
        assert_eq!(device.advance(), Some(SimAttempt::Continue(2)));
        assert_eq!(device.advance(), Some(SimAttempt::Continue(3)));
        assert_eq!(device.advance(), Some(SimAttempt::Recovered(4)));
        assert!(device.connected());
    }

    #[test]
    fn drop_stay_lost_fails_at_cap() {
        let mut device = connected_device("sim-trainer");
        assert!(device.drop_device(true));
        for attempt in 1..SIM_RECONNECT_MAX_ATTEMPTS {
            assert_eq!(device.advance(), Some(SimAttempt::Continue(attempt)));
        }
        assert_eq!(
            device.advance(),
            Some(SimAttempt::Failed(SIM_RECONNECT_MAX_ATTEMPTS))
        );
        assert!(!device.connected());
        assert!(!device.reconnecting());
    }

    #[test]
    fn retry_after_failed_restarts_attempts() {
        let mut device = connected_device("sim-trainer");
        device.drop_device(true);
        for _ in 0..SIM_RECONNECT_MAX_ATTEMPTS {
            device.advance();
        }
        assert!(device.retry());
        assert_eq!(device.advance(), Some(SimAttempt::Continue(1)));
    }

    #[test]
    fn restore_makes_next_attempt_succeed_even_when_stay_lost() {
        let mut device = connected_device("sim-trainer");
        device.drop_device(true);
        assert_eq!(device.advance(), Some(SimAttempt::Continue(1)));
        device.restore();
        assert_eq!(device.advance(), Some(SimAttempt::Recovered(2)));
        assert!(device.connected());
    }

    #[test]
    fn retry_ignored_while_reconnecting_or_without_retained_id() {
        let mut device = connected_device("sim-trainer");
        device.drop_device(false);
        assert!(!device.retry());
        let mut fresh = SimDevice::default();
        assert!(!fresh.retry());
    }

    #[test]
    fn drop_not_connected_is_noop() {
        let mut device = SimDevice::default();
        assert!(!device.drop_device(false));
        device.connect("sim-hrm".to_string());
        device.disconnect_user();
        assert!(!device.drop_device(false));
        assert!(device.id.is_none());
    }

    #[test]
    fn only_trainer_sends_ble_events() {
        assert_eq!(lost_event(Trainer), Some(BleEvent::TrainerLost));
        assert_eq!(lost_event(Hrm), None);
        assert_eq!(
            reconnected_event(Trainer),
            Some(BleEvent::TrainerReconnected)
        );
        assert_eq!(reconnected_event(Hrm), None);
    }
}
