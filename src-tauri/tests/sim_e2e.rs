//! End-to-end coverage of BLE simulation mode: the real SimActor stack is driven
//! through `BleActorHandle` over the same channels/event surfaces production uses,
//! on the tauri mock runtime with a paused tokio clock (the 1 s metric tick and the
//! 3 s reconnect cadence auto-advance instantly while keeping ordering realistic).

use opencycling_lib::ble::sim::{
    SIM_RECONNECT_INTERVAL_S, SIM_RECONNECT_MAX_ATTEMPTS, SIM_RECOVER_AFTER_ATTEMPTS,
};
use opencycling_lib::ble::{BleActorHandle, BleEvent, BleMetrics, DeviceKind};
use serde_json::Value;
use std::time::Duration;
use tauri::test::{mock_app, MockRuntime};
use tauri::{App, Listener};
use tokio::sync::mpsc::{channel, unbounded_channel, Receiver, UnboundedReceiver};

struct Harness {
    _app: App<MockRuntime>,
    ble: BleActorHandle,
    metrics_rx: Receiver<BleMetrics>,
    event_rx: Receiver<BleEvent>,
    disconnected: UnboundedReceiver<String>,
    reconnect: UnboundedReceiver<Value>,
    metrics_events: UnboundedReceiver<Value>,
}

async fn spawn_sim() -> Harness {
    let app = mock_app();
    let handle = app.handle().clone();
    let (disc_tx, disconnected) = unbounded_channel();
    let (rec_tx, reconnect) = unbounded_channel();
    let (met_tx, metrics_events) = unbounded_channel();
    let _ = handle.listen_any("ble_disconnected", move |event| {
        if let Ok(v) = serde_json::from_str::<String>(event.payload()) {
            let _ = disc_tx.send(v);
        }
    });
    let _ = handle.listen_any("ble_reconnect", move |event| {
        if let Ok(v) = serde_json::from_str::<Value>(event.payload()) {
            let _ = rec_tx.send(v);
        }
    });
    let _ = handle.listen_any("ble_metrics", move |event| {
        if let Ok(v) = serde_json::from_str::<Value>(event.payload()) {
            let _ = met_tx.send(v);
        }
    });
    let (metrics_tx, metrics_rx) = channel(64);
    let (event_tx, event_rx) = channel(16);
    let ble = BleActorHandle::spawn(handle.clone(), metrics_tx, event_tx, true)
        .await
        .expect("sim actor spawn failed");
    Harness {
        _app: app,
        ble,
        metrics_rx,
        event_rx,
        disconnected,
        reconnect,
        metrics_events,
    }
}

// Per-event wait > 2 reconnect cadences: on the paused clock the deadline is only
// reached if the expected event never comes.
fn wait_secs() -> Duration {
    Duration::from_secs(2 * SIM_RECONNECT_INTERVAL_S)
}

async fn next_metric(h: &mut Harness) -> BleMetrics {
    tokio::time::timeout(wait_secs(), h.metrics_rx.recv())
        .await
        .expect("no ble_metrics within the wait window")
        .expect("metrics channel closed")
}

async fn next_reconnect(h: &mut Harness) -> Value {
    tokio::time::timeout(wait_secs(), h.reconnect.recv())
        .await
        .expect("no ble_reconnect within the wait window")
        .expect("reconnect collector closed")
}

async fn next_disconnected(h: &mut Harness) -> String {
    tokio::time::timeout(wait_secs(), h.disconnected.recv())
        .await
        .expect("no ble_disconnected within the wait window")
        .expect("disconnected collector closed")
}

async fn next_event(h: &mut Harness) -> BleEvent {
    tokio::time::timeout(wait_secs(), h.event_rx.recv())
        .await
        .expect("no BleEvent within the wait window")
        .expect("event channel closed")
}

async fn connect_trainer_and_hrm(h: &Harness) {
    let devices = h.ble.scan().await.expect("sim scan failed");
    let trainer = devices
        .iter()
        .find(|d| d.kind == Some(DeviceKind::Trainer))
        .expect("no sim trainer in scan results");
    let hrm = devices
        .iter()
        .find(|d| d.kind == Some(DeviceKind::Hrm))
        .expect("no sim hrm in scan results");
    h.ble
        .connect_trainer(trainer.id.clone())
        .await
        .expect("trainer connect failed");
    h.ble
        .connect_hrm(hrm.id.clone())
        .await
        .expect("hrm connect failed");
}

#[tokio::test(start_paused = true)]
async fn scan_returns_sim_devices() {
    let h = spawn_sim().await;
    let devices = h.ble.scan().await.expect("sim scan failed");
    assert_eq!(devices.len(), 2);
    let trainer = devices
        .iter()
        .find(|d| d.id == "sim-trainer")
        .expect("no sim-trainer");
    let hrm = devices
        .iter()
        .find(|d| d.id == "sim-hrm")
        .expect("no sim-hrm");
    assert!(
        trainer.name.starts_with("D500"),
        "trainer name must match the D500 scan filter prefix"
    );
    assert!(matches!(trainer.kind, Some(DeviceKind::Trainer)));
    assert!(
        hrm.name.starts_with("Polar"),
        "hrm name must match the Polar scan filter prefix"
    );
    assert!(matches!(hrm.kind, Some(DeviceKind::Hrm)));
}

#[tokio::test(start_paused = true)]
async fn metrics_follow_erg_target() {
    let mut h = spawn_sim().await;
    connect_trainer_and_hrm(&h).await;
    h.ble
        .set_target_power(200)
        .await
        .expect("set_target_power failed");

    let mut prev_power = 0i16;
    let mut last_hr = 70u16;
    for _ in 0..12 {
        let m = next_metric(&mut h).await;
        let power = m.power_w.expect("trainer connected: power must be Some");
        // Raw ramp clamps at 200; the deterministic ±2 noise rides on top.
        assert!(
            power <= 202,
            "power must never overshoot beyond target + noise"
        );
        assert!(
            power >= prev_power.saturating_sub(4),
            "ramp must not retreat beyond the noise band"
        );
        prev_power = power;
        let cadence = m
            .cadence_rpm
            .expect("trainer connected: cadence must be Some");
        assert!(
            (86..=90).contains(&cadence),
            "cadence {cadence} out of band"
        );
        let hr = m.hr_bpm.expect("hrm connected: hr must be Some");
        assert!(hr >= 70, "hr {hr} below rest value");
        last_hr = hr;
    }
    // 3τ settle: ~95 % of the target reached after ~12 s of ramp.
    assert!(prev_power >= 190, "power must settle near the 200 W target");
    assert!(
        last_hr > 100,
        "hr must climb from rest 70 toward the ~195 bpm work target"
    );

    let event = tokio::time::timeout(wait_secs(), h.metrics_events.recv()).await;
    assert!(event.is_ok(), "ble_metrics must also fire as a Tauri event");
}

#[tokio::test(start_paused = true)]
async fn trainer_drop_auto_recovers() {
    let mut h = spawn_sim().await;
    connect_trainer_and_hrm(&h).await;
    h.ble
        .sim_drop_device(DeviceKind::Trainer, false)
        .await
        .expect("sim_drop failed");

    assert_eq!(next_disconnected(&mut h).await, "trainer");
    assert_eq!(next_event(&mut h).await, BleEvent::TrainerLost);

    let mut attempts = Vec::new();
    loop {
        let ev = next_reconnect(&mut h).await;
        assert_eq!(ev["device"].as_str().expect("device field"), "trainer");
        match ev["status"].as_str().expect("status field") {
            "reconnecting" => attempts.push(ev["attempt"].as_u64().expect("attempt field")),
            "reconnected" => break,
            other => panic!("unexpected ble_reconnect status: {other}"),
        }
    }
    // Recovery must land exactly at SIM_RECOVER_AFTER_ATTEMPTS.
    assert_eq!(
        attempts,
        vec![1, 2, 3, u64::from(SIM_RECOVER_AFTER_ATTEMPTS)]
    );
    assert_eq!(next_event(&mut h).await, BleEvent::TrainerReconnected);
}

#[tokio::test(start_paused = true)]
async fn trainer_drop_stay_lost_fails_then_retry_restores() {
    let mut h = spawn_sim().await;
    h.ble
        .connect_trainer("sim-trainer".to_string())
        .await
        .expect("trainer connect failed");
    h.ble
        .sim_drop_device(DeviceKind::Trainer, true)
        .await
        .expect("sim_drop failed");

    assert_eq!(next_disconnected(&mut h).await, "trainer");
    assert_eq!(next_event(&mut h).await, BleEvent::TrainerLost);

    let mut last_attempt = 0;
    loop {
        let ev = next_reconnect(&mut h).await;
        assert_eq!(ev["device"].as_str().expect("device field"), "trainer");
        match ev["status"].as_str().expect("status field") {
            "reconnecting" => last_attempt = ev["attempt"].as_u64().expect("attempt field"),
            "failed" => break,
            other => panic!("unexpected ble_reconnect status: {other}"),
        }
    }
    assert_eq!(
        last_attempt,
        u64::from(SIM_RECONNECT_MAX_ATTEMPTS),
        "giving up must happen at the reconnect attempt cap"
    );

    h.ble
        .sim_restore_device(DeviceKind::Trainer)
        .await
        .expect("restore failed");
    h.ble
        .retry_reconnect(DeviceKind::Trainer)
        .await
        .expect("retry failed");

    let first = next_reconnect(&mut h).await;
    assert_eq!(
        first["status"].as_str().expect("status field"),
        "reconnecting"
    );
    assert_eq!(
        first["attempt"].as_u64().expect("attempt field"),
        1,
        "manual retry must restart attempts at 1"
    );
    let done = next_reconnect(&mut h).await;
    assert_eq!(
        done["status"].as_str().expect("status field"),
        "reconnected"
    );
    assert_eq!(next_event(&mut h).await, BleEvent::TrainerReconnected);
}

#[tokio::test(start_paused = true)]
async fn hrm_drop_never_touches_session() {
    let mut h = spawn_sim().await;
    connect_trainer_and_hrm(&h).await;
    h.ble
        .set_target_power(150)
        .await
        .expect("set_target_power failed");
    h.ble
        .sim_drop_device(DeviceKind::Hrm, false)
        .await
        .expect("sim_drop failed");

    assert_eq!(next_disconnected(&mut h).await, "hrm");

    // Mid-outage: trainer metrics keep flowing, hr goes quiet, session untouched.
    let mut mid_outage = 0;
    for _ in 0..20 {
        let m = next_metric(&mut h).await;
        if m.hr_bpm.is_none() {
            mid_outage += 1;
            assert!(
                m.power_w.is_some(),
                "trainer power must keep flowing during the hrm outage"
            );
        }
    }
    assert!(
        mid_outage >= 3,
        "expected hr-less metrics during the hrm outage"
    );
    assert!(
        h.event_rx.try_recv().is_err(),
        "hrm loss must never emit a BleEvent"
    );

    let mut saw_reconnecting = false;
    loop {
        let ev = next_reconnect(&mut h).await;
        assert_eq!(ev["device"].as_str().expect("device field"), "hrm");
        match ev["status"].as_str().expect("status field") {
            "reconnecting" => saw_reconnecting = true,
            "reconnected" => break,
            other => panic!("unexpected ble_reconnect status: {other}"),
        }
    }
    assert!(
        saw_reconnecting,
        "hrm must go through reconnecting before reconnected"
    );
    assert!(
        h.event_rx.try_recv().is_err(),
        "hrm recovery must never emit a BleEvent"
    );
}

#[tokio::test(start_paused = true)]
async fn drop_of_unconnected_device_is_noop() {
    let mut h = spawn_sim().await;
    h.ble
        .sim_drop_device(DeviceKind::Trainer, false)
        .await
        .expect("sim_drop failed");
    // Advance well past a reconnect cadence: a wrongly spawned loop would have ticked.
    tokio::time::sleep(Duration::from_secs(2 * SIM_RECONNECT_INTERVAL_S)).await;
    assert!(h.event_rx.try_recv().is_err(), "no BleEvent expected");
    assert!(
        h.disconnected.try_recv().is_err(),
        "no ble_disconnected expected"
    );
    assert!(h.reconnect.try_recv().is_err(), "no ble_reconnect expected");
}
