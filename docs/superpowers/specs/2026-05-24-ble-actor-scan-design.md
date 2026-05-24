# BleActor — Scan only (phase 1)

## Context

BleActor is the Tokio actor that owns all BLE state in OpenCycling. This spec covers only the `Scan` command — the foundational pattern on which `ConnectTrainer`, `ConnectHrm`, and ERG keep-alive will be added in later phases.

## Scope

**In scope:**
- `BleActor` struct with `cmd_rx` and `btleplug` adapter
- `BleActor::run()` — async loop with `tokio::select!`
- `BleActorHandle` — the `mpsc::Sender` wrapper exposed to Tauri commands
- `BleActorHandle::spawn()` — initializes btleplug, creates channel, spawns task
- `scan_devices()` Tauri command — sends `Scan` via channel, awaits `oneshot` reply
- Register `BleActorHandle` as Tauri managed state in `lib.rs`

**Out of scope (next phases):**
- `ConnectTrainer`, `ConnectHrm`, `SetTargetPower`
- ERG keep-alive timer
- `ble_metrics` and `ble_error` events

## File Layout

```
src-tauri/
  Cargo.toml              ← add btleplug
  src/ble/
    types.rs              ← BleCommand enum, BleActor struct, BleMetrics, BleError, DeviceInfo
    actor.rs              ← BleActor::run(), handle_scan()
    command.rs            ← BleActorHandle, spawn(), scan_devices Tauri command helper
    mod.rs                ← pub use BleActorHandle
  src/lib.rs              ← spawn actor, register state, add scan_devices command
```

## Data Flow — Scan

```
Tauri command                 mpsc channel            BleActor task
scan_devices(app)
  1. oneshot::channel() → (tx, rx)
  2. sender.send(Scan{reply:tx})  ──────────────────► select! branch fires
                                                        handle_scan(adapter, reply)
                                                          btleplug: start_scan 2s
                                                          collect peripherals
                                                          filter by "D500" | "Polar"
  3. rx.await ◄───────────────────────────────────────   reply.send(Vec<DeviceInfo>)
  4. return Vec<DeviceInfo>
```

## Key Types

```rust
// types.rs — already partially defined
#[derive(Serialize, Clone)]
pub enum DeviceKind { Trainer, Hrm }

#[derive(Serialize, Clone)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,  // set by Rust at scan time — frontend doesn't re-parse names
}

pub enum BleCommand {
    Scan { reply: oneshot::Sender<Result<Vec<DeviceInfo>, AppError>> }, // Result: distinguishes "found nothing" from "adapter failed"
    ConnectTrainer { device_id: String, reply: oneshot::Sender<Result<(), AppError>> },
    ConnectHrm     { device_id: String, reply: oneshot::Sender<Result<(), AppError>> },
    SetTargetPower { watts: i16 },
}

pub struct BleActor {
    cmd_rx:   mpsc::Receiver<BleCommand>,
    adapter:  btleplug::platform::Adapter,
    _manager: btleplug::platform::Manager, // kept alive — adapter is invalid without it
}

// command.rs — new
pub struct BleActorHandle {
    sender: mpsc::Sender<BleCommand>,
}
```

## BleActor::run() shape

```rust
pub async fn run(mut self) {
    loop {
        tokio::select! {
            Some(cmd) = self.cmd_rx.recv() => match cmd {
                BleCommand::Scan { reply } => handle_scan(&self.adapter, reply).await,
                _ => {} // ConnectTrainer, ConnectHrm, SetTargetPower: not yet implemented
            }
        }
    }
}
```

## BleActorHandle::spawn()

```rust
impl BleActorHandle {
    pub async fn spawn() -> Result<Self, AppError> {
        let manager  = Manager::new().await?;
        let adapter  = manager.adapters().await?.into_iter().next()
            .ok_or(AppError::DeviceNotFound)?;
        let (tx, rx) = mpsc::channel(32);
        let actor    = BleActor { cmd_rx: rx, adapter, _manager: manager };
        tokio::spawn(actor.run());
        Ok(Self { sender: tx })
    }
}
```

## Tauri wiring (lib.rs)

```rust
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = tauri::async_runtime::block_on(BleActorHandle::spawn())
                .expect("BLE init failed");
            app.manage(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![scan_devices])
        .run(tauri::generate_context!())
        .expect("error running tauri");
}

#[tauri::command]
async fn scan_devices(state: tauri::State<'_, BleActorHandle>) -> Result<Vec<DeviceInfo>, AppError> {
    state.scan().await
}
```

## Acceptance Criteria

- [ ] `scan_devices()` returns a `Vec<DeviceInfo>` filtered by name prefix (`"D500"` or `"Polar"`)
- [ ] BleActor runs as a Tokio task — no blocking in Tauri command thread
- [ ] `BleActorHandle` is registered as Tauri managed state
- [ ] Other `BleCommand` variants compile but are ignored (`_ => {}`)
- [ ] `cargo clippy` passes with no warnings

## Rust Concepts Introduced (learning checkpoints)

| Concept | Where it appears |
|---------|-----------------|
| `mpsc::channel` | BleActorHandle::spawn() |
| `oneshot::channel` | scan_devices() + BleCommand::Scan |
| `tokio::select!` | BleActor::run() |
| `tokio::spawn` | BleActorHandle::spawn() |
| `tauri::State<T>` | scan_devices command signature |