# Phase 1 & 2 — BLE Foundation + ERG Control

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connect to the Van Rysel D500 and Polar H10 over BLE, read real-time power/cadence/HR, and send ERG target power commands — all displayed in a minimal Svelte UI.

**Architecture:** Three Rust actors (BleActor, SessionActor placeholder, DbActor placeholder) communicating over `mpsc` channels. BleActor owns all BLE state. Tauri commands delegate to actor Handles. Frontend is purely reactive, driven by Tauri events.

**Tech Stack:** Rust + Tauri v2, btleplug 0.11 (winrt feature), tokio, Svelte 5 (runes), SvelteKit with adapter-static, TypeScript.

---

## File Map

```
opencycling/
├── src-tauri/
│   ├── build.rs
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs          # minimal: calls lib::run()
│       ├── lib.rs           # Tauri builder, state init, command registration
│       ├── error.rs         # AppError type
│       ├── commands.rs      # all #[tauri::command] handlers
│       └── ble/
│           ├── mod.rs       # BleActor, BleHandle, BleEvent, BleCommand
│           ├── ftms.rs      # FTMS data parsing + ERG command builder
│           └── hrs.rs       # HRS data parsing
├── src/
│   ├── app.css
│   ├── routes/
│   │   ├── +layout.svelte   # event listener setup (listen once at app start)
│   │   ├── +layout.ts       # export prerender=true, ssr=false
│   │   └── +page.svelte     # dashboard: scan, connect, live metrics, watt slider
│   └── lib/
│       └── stores/
│           ├── devices.svelte.ts  # trainer/hrm connection state
│           └── session.svelte.ts  # live metrics (power, hr, cadence)
├── svelte.config.js
├── vite.config.ts
└── package.json
```

---

## Task 1: Project scaffold

**Files:**
- Restructure: move existing Rust into `src-tauri/`
- Create: SvelteKit frontend in root

- [ ] **Step 1: Move existing Rust into src-tauri/**

Run in PowerShell from the `opencycling/` directory:

```powershell
New-Item -ItemType Directory -Path src-tauri
New-Item -ItemType Directory -Path src-tauri\src
New-Item -ItemType Directory -Path src-tauri\icons
Move-Item Cargo.lock src-tauri\Cargo.lock
```

The existing `Cargo.toml` and `src\main.rs` will be replaced in Task 2 — delete them now:

```powershell
Remove-Item Cargo.toml
Remove-Item -Recurse src
```

- [ ] **Step 2: Create SvelteKit frontend**

```powershell
npx sv create . --template minimal --types ts --no-add-ons
```

When prompted about the existing directory, accept. This creates `src/`, `package.json`, `svelte.config.js`, `vite.config.ts`.

- [ ] **Step 3: Install npm dependencies**

```powershell
npm install
npm install @tauri-apps/api@2
npm install -D @tauri-apps/cli@2 @sveltejs/adapter-static
```

- [ ] **Step 4: Commit scaffold**

```powershell
git add package.json package-lock.json svelte.config.js vite.config.ts src/
git commit -m "chore: scaffold SvelteKit frontend"
```

---

## Task 2: Rust workspace + Tauri config

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/main.rs`

- [ ] **Step 1: Write src-tauri/Cargo.toml**

```toml
[package]
name = "opencycling"
version = "0.1.0"
edition = "2021"

[lib]
name = "opencycling_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
btleplug = { version = "0.11", features = ["winrt"] }
thiserror = "1"
log = "0.4"
env_logger = "0.11"
```

- [ ] **Step 2: Write src-tauri/build.rs**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 3: Write src-tauri/tauri.conf.json**

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "OpenCycling",
  "version": "0.1.0",
  "identifier": "com.opencycling.app",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:5173",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../build"
  },
  "app": {
    "windows": [
      {
        "title": "OpenCycling",
        "width": 1280,
        "height": 800,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/icon.ico"]
  }
}
```

- [ ] **Step 4: Write src-tauri/src/main.rs**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    opencycling_lib::run()
}
```

- [ ] **Step 5: Update svelte.config.js for static adapter**

Replace the generated `svelte.config.js` with:

```javascript
import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({ fallback: 'index.html' })
  }
};

export default config;
```

- [ ] **Step 6: Verify Rust compiles**

```powershell
cd src-tauri
cargo build
cd ..
```

Expected: compiles with no errors (may have warnings about unused items).

- [ ] **Step 7: Commit**

```powershell
git add src-tauri/ svelte.config.js
git commit -m "chore: add Tauri v2 config and Rust workspace"
```

---

## Task 3: Error type + module structure

**Files:**
- Create: `src-tauri/src/error.rs`
- Create: `src-tauri/src/lib.rs` (minimal)
- Create: `src-tauri/src/commands.rs` (stub)
- Create: `src-tauri/src/ble/mod.rs` (stub)
- Create: `src-tauri/src/ble/ftms.rs` (stub)
- Create: `src-tauri/src/ble/hrs.rs` (stub)

- [ ] **Step 1: Write src-tauri/src/error.rs**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("BLE error: {0}")]
    Ble(#[from] btleplug::Error),
    #[error("Device not found")]
    DeviceNotFound,
    #[error("Characteristic not found: {0}")]
    CharacteristicNotFound(String),
    #[error("Actor channel closed")]
    ChannelClosed,
    #[error("{0}")]
    Other(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(&self.to_string())
    }
}
```

- [ ] **Step 2: Write stub src-tauri/src/ble/ftms.rs**

```rust
// FTMS Indoor Bike Data parser and ERG command builder.
// Parsing logic tested in the ftms tests module below.

pub struct IndoorBikeData {
    pub speed_kmh: f32,
    pub cadence_rpm: f32,
    pub power_w: i16,
}

pub fn parse_indoor_bike_data(_data: &[u8]) -> Option<IndoorBikeData> {
    todo!()
}

pub fn build_set_target_power_cmd(_watts: u16) -> [u8; 3] {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn test_placeholder() {
        parse_indoor_bike_data(&[]);
    }
}
```

- [ ] **Step 3: Write stub src-tauri/src/ble/hrs.rs**

```rust
pub fn parse_hr_measurement(_data: &[u8]) -> Option<u16> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn test_placeholder() {
        parse_hr_measurement(&[]);
    }
}
```

- [ ] **Step 4: Write stub src-tauri/src/ble/mod.rs**

```rust
pub mod ftms;
pub mod hrs;
```

- [ ] **Step 5: Write stub src-tauri/src/commands.rs**

```rust
// Tauri command handlers — implemented in later tasks.
```

- [ ] **Step 6: Write src-tauri/src/lib.rs**

```rust
mod ble;
mod commands;
mod error;

pub fn run() {
    env_logger::init();
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 7: Verify compile**

```powershell
cd src-tauri && cargo build && cd ..
```

Expected: compiles.

- [ ] **Step 8: Commit**

```powershell
git add src-tauri/src/
git commit -m "chore: add module structure and error type"
```

---

## Task 4: FTMS data parser (TDD)

**Files:**
- Modify: `src-tauri/src/ble/ftms.rs`

FTMS Indoor Bike Data (UUID `0x2AD2`) packet format. The Flags field (2 bytes, LE) controls which fields follow:
- Bit 0: `More Data` — when **0**, Instantaneous Speed is present (2 bytes, 0.01 km/h)
- Bit 2: Instantaneous Cadence present (2 bytes, 0.5 rpm)
- Bit 6: Instantaneous Power present (2 bytes, signed int16, watts)

Fields appear in flag-bit order after the 2-byte flags. Skipped optional fields still advance the offset (Average Speed bit1 = 2 bytes, Average Cadence bit3 = 2 bytes, Total Distance bit4 = 3 bytes, Resistance Level bit5 = 2 bytes).

**Note:** Verify the D500's actual flag values with nRF Connect before trusting this parser. The D500 may not set all bits — use nRF Connect to capture a raw notification and decode it manually to validate.

- [ ] **Step 1: Write the failing tests first**

Replace `src-tauri/src/ble/ftms.rs` with:

```rust
pub struct IndoorBikeData {
    pub speed_kmh: f32,
    pub cadence_rpm: f32,
    pub power_w: i16,
}

pub fn parse_indoor_bike_data(data: &[u8]) -> Option<IndoorBikeData> {
    todo!()
}

pub fn build_set_target_power_cmd(watts: u16) -> [u8; 3] {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Packet with speed + power only (flags = 0x0041: bit0=1 no speed, bit6=1 power)
    // Wait — bit0=1 means speed NOT present. So flags=0x40 = bit6 only = power only.
    // flags=0x0040: power present, speed absent (bit0=1 would mean more data / no speed)
    // flags=0x0000: speed present only
    // flags=0x0044: cadence (bit2) + power (bit6)
    // flags=0x0001: bit0=1 meaning speed NOT present

    #[test]
    fn test_parse_power_only() {
        // flags = 0x0040 (bit 6 set = power, bit 0 = 0 so speed IS present)
        // speed: 100 * 0.01 = 1.00 km/h → raw = 100 = [0x64, 0x00]
        // power: 200W → raw = 200 = [0xC8, 0x00]
        let data = [
            0x40, 0x00, // flags: bit6 (power) set, bit0=0 (speed present)
            0x64, 0x00, // speed: 100 → 1.00 km/h
            0xC8, 0x00, // power: 200W
        ];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert_eq!(result.power_w, 200);
        assert!((result.speed_kmh - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_speed_cadence_power() {
        // flags = 0x0044: bit2 (cadence) + bit6 (power), bit0=0 (speed present)
        // speed: 200 → 2.00 km/h = [0xC8, 0x00]
        // cadence: 180 raw → 90 rpm (×0.5) = [0xB4, 0x00]
        // power: 150W = [0x96, 0x00]
        let data = [
            0x44, 0x00, // flags: bit2 + bit6
            0xC8, 0x00, // speed: 2.00 km/h
            0xB4, 0x00, // cadence: 90 rpm
            0x96, 0x00, // power: 150W
        ];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert_eq!(result.power_w, 150);
        assert!((result.cadence_rpm - 90.0).abs() < 0.1);
        assert!((result.speed_kmh - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_negative_power() {
        // Negative power shouldn't happen on a trainer but the field is signed
        // flags = 0x40 + speed
        let power: i16 = -5;
        let power_bytes = power.to_le_bytes();
        let data = [
            0x40, 0x00,
            0x00, 0x00, // speed = 0
            power_bytes[0], power_bytes[1],
        ];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert_eq!(result.power_w, -5);
    }

    #[test]
    fn test_parse_empty_returns_none() {
        assert!(parse_indoor_bike_data(&[]).is_none());
    }

    #[test]
    fn test_parse_too_short_returns_none() {
        assert!(parse_indoor_bike_data(&[0x40]).is_none());
    }

    #[test]
    fn test_build_set_target_power_200w() {
        // 200W = [0x05, 0xC8, 0x00]
        assert_eq!(build_set_target_power_cmd(200), [0x05, 0xC8, 0x00]);
    }

    #[test]
    fn test_build_set_target_power_0w() {
        assert_eq!(build_set_target_power_cmd(0), [0x05, 0x00, 0x00]);
    }

    #[test]
    fn test_build_set_target_power_400w() {
        // 400 = 0x0190
        assert_eq!(build_set_target_power_cmd(400), [0x05, 0x90, 0x01]);
    }
}
```

- [ ] **Step 2: Run tests — verify they fail (todo! panics)**

```powershell
cd src-tauri && cargo test ble::ftms && cd ..
```

Expected: FAIL — panics at `todo!()`.

- [ ] **Step 3: Implement parse_indoor_bike_data and build_set_target_power_cmd**

Replace the two `todo!()` function bodies:

```rust
pub fn parse_indoor_bike_data(data: &[u8]) -> Option<IndoorBikeData> {
    if data.len() < 2 {
        return None;
    }
    let flags = u16::from_le_bytes([data[0], data[1]]);
    let mut offset = 2usize;

    // Bit 0 = 0: Instantaneous Speed present (2 bytes, 0.01 km/h)
    let speed_kmh = if flags & 0x01 == 0 {
        if offset + 2 > data.len() { return None; }
        let raw = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        raw as f32 * 0.01
    } else {
        0.0
    };

    // Bit 1: Average Speed (2 bytes) — skip
    if flags & 0x02 != 0 { offset += 2; }

    // Bit 2: Instantaneous Cadence (2 bytes, 0.5 rpm)
    let cadence_rpm = if flags & 0x04 != 0 {
        if offset + 2 > data.len() { return None; }
        let raw = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        raw as f32 * 0.5
    } else {
        0.0
    };

    // Bit 3: Average Cadence — skip
    if flags & 0x08 != 0 { offset += 2; }
    // Bit 4: Total Distance — skip (3 bytes)
    if flags & 0x10 != 0 { offset += 3; }
    // Bit 5: Resistance Level — skip
    if flags & 0x20 != 0 { offset += 2; }

    // Bit 6: Instantaneous Power (2 bytes, signed int16, watts)
    let power_w = if flags & 0x40 != 0 {
        if offset + 2 > data.len() { return None; }
        i16::from_le_bytes([data[offset], data[offset + 1]])
    } else {
        0
    };

    Some(IndoorBikeData { speed_kmh, cadence_rpm, power_w })
}

pub fn build_set_target_power_cmd(watts: u16) -> [u8; 3] {
    let bytes = (watts as i16).to_le_bytes();
    [0x05, bytes[0], bytes[1]]
}
```

- [ ] **Step 4: Run tests — verify they pass**

```powershell
cd src-tauri && cargo test ble::ftms && cd ..
```

Expected: all 8 tests PASS.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/ble/ftms.rs
git commit -m "feat: FTMS indoor bike data parser and ERG command builder"
```

---

## Task 5: HRS data parser (TDD)

**Files:**
- Modify: `src-tauri/src/ble/hrs.rs`

HRS Heart Rate Measurement (UUID `0x2A37`). Byte 0 = flags. Bit 0 of flags: 0 = BPM is uint8 at byte 1; 1 = BPM is uint16 LE at bytes 1-2.

- [ ] **Step 1: Write failing tests**

Replace `src-tauri/src/ble/hrs.rs`:

```rust
pub fn parse_hr_measurement(data: &[u8]) -> Option<u16> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bpm_uint8() {
        // flags bit0 = 0 → BPM is u8 at byte 1
        let data = [0x00, 0x48]; // 0x48 = 72 bpm
        assert_eq!(parse_hr_measurement(&data), Some(72));
    }

    #[test]
    fn test_bpm_uint16() {
        // flags bit0 = 1 → BPM is u16 LE at bytes 1-2
        let data = [0x01, 0xC8, 0x00]; // 200 bpm (high intensity!)
        assert_eq!(parse_hr_measurement(&data), Some(200));
    }

    #[test]
    fn test_typical_polar_packet() {
        // Polar H10 typically sends uint8 BPM with some extra fields
        // flags = 0x10 (energy expended present, bit4) — bit0=0 so uint8 BPM
        let data = [0x10, 0x4B, 0x00, 0x00]; // 75 bpm + 2 bytes energy
        assert_eq!(parse_hr_measurement(&data), Some(75));
    }

    #[test]
    fn test_empty_returns_none() {
        assert_eq!(parse_hr_measurement(&[]), None);
    }

    #[test]
    fn test_too_short_for_uint16_returns_none() {
        // flags say uint16 but only 1 byte of data
        let data = [0x01, 0x50]; // need 3 bytes total
        assert_eq!(parse_hr_measurement(&data), None);
    }
}
```

- [ ] **Step 2: Run — verify they fail**

```powershell
cd src-tauri && cargo test ble::hrs && cd ..
```

Expected: FAIL (todo! panics).

- [ ] **Step 3: Implement**

```rust
pub fn parse_hr_measurement(data: &[u8]) -> Option<u16> {
    if data.is_empty() {
        return None;
    }
    let flags = data[0];
    if flags & 0x01 == 0 {
        data.get(1).map(|&b| b as u16)
    } else {
        if data.len() < 3 { return None; }
        Some(u16::from_le_bytes([data[1], data[2]]))
    }
}
```

- [ ] **Step 4: Run — verify they pass**

```powershell
cd src-tauri && cargo test ble::hrs && cd ..
```

Expected: all 5 tests PASS.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/ble/hrs.rs
git commit -m "feat: HRS heart rate measurement parser"
```

---

## Task 6: BleActor — messages and Handle

**Files:**
- Modify: `src-tauri/src/ble/mod.rs`

- [ ] **Step 1: Write the message types and Handle**

Replace `src-tauri/src/ble/mod.rs`:

```rust
pub mod ftms;
pub mod hrs;

use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time;

use crate::error::AppError;
use ftms::{build_set_target_power_cmd, parse_indoor_bike_data};
use hrs::parse_hr_measurement;

// UUIDs
const FTMS_SERVICE: &str = "00001826-0000-1000-8000-00805f9b34fb";
const HRS_SERVICE:  &str = "0000180d-0000-1000-8000-00805f9b34fb";

const INDOOR_BIKE_DATA_UUID:   &str = "00002ad2-0000-1000-8000-00805f9b34fb";
const CONTROL_POINT_UUID:      &str = "00002ad9-0000-1000-8000-00805f9b34fb";
const HRM_MEASUREMENT_UUID:    &str = "00002a37-0000-1000-8000-00805f9b34fb";

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceKind {
    Trainer,
    Hrm,
    Unknown,
}

#[derive(Debug)]
pub enum BleCommand {
    Scan { reply: oneshot::Sender<Vec<DeviceInfo>> },
    ConnectTrainer { id: String, reply: oneshot::Sender<Result<(), AppError>> },
    ConnectHrm { id: String, reply: oneshot::Sender<Result<(), AppError>> },
    SetTargetPower { watts: u16 },
    RequestControlAndStart { reply: oneshot::Sender<Result<(), AppError>> },
    DisconnectAll { reply: oneshot::Sender<()> },
}

/// Clone-able handle that any Tauri command handler can use to talk to BleActor.
#[derive(Clone)]
pub struct BleHandle {
    tx: mpsc::Sender<BleCommand>,
}

impl BleHandle {
    pub async fn scan(&self) -> Vec<DeviceInfo> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(BleCommand::Scan { reply: tx }).await;
        rx.await.unwrap_or_default()
    }

    pub async fn connect_trainer(&self, id: String) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(BleCommand::ConnectTrainer { id, reply: tx }).await;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn connect_hrm(&self, id: String) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(BleCommand::ConnectHrm { id, reply: tx }).await;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn set_target_power(&self, watts: u16) {
        let _ = self.tx.send(BleCommand::SetTargetPower { watts }).await;
    }

    pub async fn request_control_and_start(&self) -> Result<(), AppError> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(BleCommand::RequestControlAndStart { reply: tx }).await;
        rx.await.map_err(|_| AppError::ChannelClosed)?
    }

    pub async fn disconnect_all(&self) {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(BleCommand::DisconnectAll { reply: tx }).await;
        let _ = rx.await;
    }
}

pub struct BleActor {
    rx: mpsc::Receiver<BleCommand>,
    adapter: Option<Adapter>,
    trainer: Option<Peripheral>,
    hrm: Option<Peripheral>,
    app_handle: tauri::AppHandle,
}

impl BleActor {
    /// Spawn the actor and return a Handle to communicate with it.
    pub fn spawn(app_handle: tauri::AppHandle) -> BleHandle {
        let (tx, rx) = mpsc::channel(32);
        let actor = BleActor {
            rx,
            adapter: None,
            trainer: None,
            hrm: None,
            app_handle,
        };
        tokio::spawn(actor.run());
        BleHandle { tx }
    }

    async fn run(mut self) {
        // Initialize BLE adapter
        match Manager::new().await {
            Ok(manager) => {
                match manager.adapters().await {
                    Ok(adapters) if !adapters.is_empty() => {
                        self.adapter = Some(adapters.into_iter().next().unwrap());
                        log::info!("BLE adapter initialized");
                    }
                    _ => log::error!("No BLE adapters found"),
                }
            }
            Err(e) => log::error!("Failed to init BLE manager: {e}"),
        }

        while let Some(cmd) = self.rx.recv().await {
            self.handle_command(cmd).await;
        }
    }

    async fn handle_command(&mut self, cmd: BleCommand) {
        match cmd {
            BleCommand::Scan { reply } => {
                let devices = self.scan().await;
                let _ = reply.send(devices);
            }
            BleCommand::ConnectTrainer { id, reply } => {
                let result = self.connect_trainer(&id).await;
                let _ = reply.send(result);
            }
            BleCommand::ConnectHrm { id, reply } => {
                let result = self.connect_hrm(&id).await;
                let _ = reply.send(result);
            }
            BleCommand::SetTargetPower { watts } => {
                self.set_target_power(watts).await;
            }
            BleCommand::RequestControlAndStart { reply } => {
                let result = self.request_control_and_start().await;
                let _ = reply.send(result);
            }
            BleCommand::DisconnectAll { reply } => {
                self.disconnect_all().await;
                let _ = reply.send(());
            }
        }
    }

    async fn scan(&mut self) -> Vec<DeviceInfo> {
        let Some(adapter) = &self.adapter else { return vec![]; };

        if let Err(e) = adapter.start_scan(ScanFilter::default()).await {
            log::error!("Scan start failed: {e}");
            return vec![];
        }

        time::sleep(Duration::from_secs(5)).await;
        let _ = adapter.stop_scan().await;

        let peripherals = adapter.peripherals().await.unwrap_or_default();
        let mut devices = Vec::new();

        for p in peripherals {
            let Ok(Some(props)) = p.properties().await else { continue };
            let name = props.local_name.unwrap_or_else(|| "Unknown".into());
            let services: Vec<String> = props.services.iter()
                .map(|u| u.to_string().to_lowercase())
                .collect();

            let kind = if services.contains(&FTMS_SERVICE.to_lowercase()) {
                DeviceKind::Trainer
            } else if services.contains(&HRS_SERVICE.to_lowercase()) {
                DeviceKind::Hrm
            } else {
                DeviceKind::Unknown
            };

            devices.push(DeviceInfo {
                id: p.id().to_string(),
                name,
                kind,
            });
        }

        devices
    }

    async fn connect_trainer(&mut self, id: &str) -> Result<(), AppError> {
        let adapter = self.adapter.as_ref().ok_or(AppError::DeviceNotFound)?;
        let peripherals = adapter.peripherals().await?;
        let peripheral = peripherals.into_iter()
            .find(|p| p.id().to_string() == id)
            .ok_or(AppError::DeviceNotFound)?;

        peripheral.connect().await?;
        peripheral.discover_services().await?;

        // Subscribe to Indoor Bike Data notifications
        let chars = peripheral.characteristics();
        let ibd_char = chars.iter()
            .find(|c| c.uuid.to_string().to_lowercase() == INDOOR_BIKE_DATA_UUID)
            .ok_or_else(|| AppError::CharacteristicNotFound(INDOOR_BIKE_DATA_UUID.into()))?
            .clone();

        peripheral.subscribe(&ibd_char).await?;

        // Spawn notification listener
        let mut notifications = peripheral.notifications().await?;
        let app_handle = self.app_handle.clone();
        let ibd_uuid = ibd_char.uuid;

        tokio::spawn(async move {
            use futures::StreamExt;
            while let Some(notif) = notifications.next().await {
                if notif.uuid == ibd_uuid {
                    if let Some(data) = parse_indoor_bike_data(&notif.value) {
                        let _ = app_handle.emit("metrics_ble", serde_json::json!({
                            "power": data.power_w.max(0) as u32,
                            "cadence": data.cadence_rpm as u32,
                            "speed_kmh": data.speed_kmh,
                        }));
                    }
                }
            }
        });

        self.trainer = Some(peripheral);
        self.app_handle.emit("ble", serde_json::json!({ "trainer": "connected" })).ok();
        log::info!("Trainer connected");
        Ok(())
    }

    async fn connect_hrm(&mut self, id: &str) -> Result<(), AppError> {
        let adapter = self.adapter.as_ref().ok_or(AppError::DeviceNotFound)?;
        let peripherals = adapter.peripherals().await?;
        let peripheral = peripherals.into_iter()
            .find(|p| p.id().to_string() == id)
            .ok_or(AppError::DeviceNotFound)?;

        peripheral.connect().await?;
        peripheral.discover_services().await?;

        let chars = peripheral.characteristics();
        let hrm_char = chars.iter()
            .find(|c| c.uuid.to_string().to_lowercase() == HRM_MEASUREMENT_UUID)
            .ok_or_else(|| AppError::CharacteristicNotFound(HRM_MEASUREMENT_UUID.into()))?
            .clone();

        peripheral.subscribe(&hrm_char).await?;

        let mut notifications = peripheral.notifications().await?;
        let app_handle = self.app_handle.clone();
        let hrm_uuid = hrm_char.uuid;

        tokio::spawn(async move {
            use futures::StreamExt;
            while let Some(notif) = notifications.next().await {
                if notif.uuid == hrm_uuid {
                    if let Some(bpm) = parse_hr_measurement(&notif.value) {
                        let _ = app_handle.emit("metrics_ble", serde_json::json!({ "hr": bpm }));
                    }
                }
            }
        });

        self.hrm = Some(peripheral);
        self.app_handle.emit("ble", serde_json::json!({ "hrm": "connected" })).ok();
        log::info!("HRM connected");
        Ok(())
    }

    async fn request_control_and_start(&mut self) -> Result<(), AppError> {
        let trainer = self.trainer.as_ref().ok_or(AppError::DeviceNotFound)?;
        let chars = trainer.characteristics();

        let cp_char = chars.iter()
            .find(|c| c.uuid.to_string().to_lowercase() == CONTROL_POINT_UUID)
            .ok_or_else(|| AppError::CharacteristicNotFound(CONTROL_POINT_UUID.into()))?
            .clone();

        // Subscribe to indications on control point (required before writing)
        trainer.subscribe(&cp_char).await?;

        // Step 1: Request Control
        trainer.write(&cp_char, &[0x00], WriteType::WithResponse).await?;
        time::sleep(Duration::from_millis(200)).await;

        // Step 2: Start session
        trainer.write(&cp_char, &[0x07], WriteType::WithResponse).await?;
        time::sleep(Duration::from_millis(200)).await;

        log::info!("FTMS control acquired");
        Ok(())
    }

    async fn set_target_power(&self, watts: u16) {
        let Some(trainer) = &self.trainer else { return };
        let chars = trainer.characteristics();
        let Some(cp_char) = chars.iter()
            .find(|c| c.uuid.to_string().to_lowercase() == CONTROL_POINT_UUID)
            .cloned()
        else { return };

        let cmd = build_set_target_power_cmd(watts);
        if let Err(e) = trainer.write(&cp_char, &cmd, WriteType::WithResponse).await {
            log::error!("ERG write failed: {e}");
        }
    }

    async fn disconnect_all(&mut self) {
        if let Some(t) = self.trainer.take() {
            let _ = t.disconnect().await;
        }
        if let Some(h) = self.hrm.take() {
            let _ = h.disconnect().await;
        }
        self.app_handle.emit("ble", serde_json::json!({
            "trainer": "disconnected",
            "hrm": "disconnected"
        })).ok();
    }
}
```

- [ ] **Step 2: Add futures dependency to Cargo.toml**

Add to `[dependencies]` in `src-tauri/Cargo.toml`:
```toml
futures = "0.3"
```

- [ ] **Step 3: Verify compile**

```powershell
cd src-tauri && cargo build && cd ..
```

Expected: compiles (may have unused import warnings — fine).

- [ ] **Step 4: Commit**

```powershell
git add src-tauri/src/ble/mod.rs src-tauri/Cargo.toml
git commit -m "feat: BleActor with scan, connect, FTMS+HRS notifications"
```

---

## Task 7: Tauri commands + app state

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write commands.rs**

```rust
use tauri::State;
use crate::ble::{BleHandle, DeviceInfo};
use crate::error::AppError;

pub struct AppState {
    pub ble: BleHandle,
}

#[tauri::command]
pub async fn scan_devices(state: State<'_, AppState>) -> Result<Vec<DeviceInfo>, AppError> {
    Ok(state.ble.scan().await)
}

#[tauri::command]
pub async fn connect_trainer(
    device_id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.ble.connect_trainer(device_id).await
}

#[tauri::command]
pub async fn connect_hrm(
    device_id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.ble.connect_hrm(device_id).await
}

#[tauri::command]
pub async fn disconnect_all(state: State<'_, AppState>) -> Result<(), AppError> {
    state.ble.disconnect_all().await;
    Ok(())
}

#[tauri::command]
pub async fn request_control_and_start(state: State<'_, AppState>) -> Result<(), AppError> {
    state.ble.request_control_and_start().await
}

#[tauri::command]
pub async fn set_target_power(
    watts: u16,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state.ble.set_target_power(watts).await;
    Ok(())
}
```

- [ ] **Step 2: Update lib.rs**

```rust
mod ble;
mod commands;
mod error;

use commands::AppState;
use ble::BleActor;

pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .setup(|app| {
            let ble_handle = BleActor::spawn(app.handle().clone());
            app.manage(AppState { ble: ble_handle });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan_devices,
            commands::connect_trainer,
            commands::connect_hrm,
            commands::disconnect_all,
            commands::request_control_and_start,
            commands::set_target_power,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Verify compile**

```powershell
cd src-tauri && cargo build && cd ..
```

Expected: compiles.

- [ ] **Step 4: Commit**

```powershell
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: Tauri commands and app state wiring"
```

---

## Task 8: Frontend stores

**Files:**
- Create: `src/routes/+layout.ts`
- Create: `src/lib/stores/devices.svelte.ts`
- Create: `src/lib/stores/session.svelte.ts`

- [ ] **Step 1: Create src/routes/+layout.ts**

```typescript
export const prerender = true;
export const ssr = false;
```

- [ ] **Step 2: Create src/lib/stores/devices.svelte.ts**

```typescript
type ConnectionStatus = 'connected' | 'disconnected' | 'connecting';

export const devices = $state({
  trainer: 'disconnected' as ConnectionStatus,
  hrm: 'disconnected' as ConnectionStatus,
});
```

- [ ] **Step 3: Create src/lib/stores/session.svelte.ts**

```typescript
export const metrics = $state({
  power: 0,
  target: 0,
  hr: 0,
  cadence: 0,
  elapsed_s: 0,
  remaining_s: 0,
  total_remaining_s: 0,
});
```

- [ ] **Step 4: Verify TypeScript compiles**

```powershell
npm run check
```

Expected: no errors.

- [ ] **Step 5: Commit**

```powershell
git add src/routes/+layout.ts src/lib/
git commit -m "feat: Svelte 5 rune stores for device and session state"
```

---

## Task 9: Frontend layout — event listeners

**Files:**
- Create: `src/routes/+layout.svelte`

- [ ] **Step 1: Write +layout.svelte**

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { devices } from '$lib/stores/devices.svelte';
  import { metrics } from '$lib/stores/session.svelte';
  import '../app.css';

  let { children } = $props();

  onMount(async () => {
    // BLE metrics coming from FTMS/HRS notifications
    await listen<{ power?: number; hr?: number; cadence?: number }>('metrics_ble', (e) => {
      if (e.payload.power !== undefined) metrics.power = e.payload.power;
      if (e.payload.hr !== undefined) metrics.hr = e.payload.hr;
      if (e.payload.cadence !== undefined) metrics.cadence = e.payload.cadence;
    });

    // Full metrics from SessionActor (Phase 3+)
    await listen<typeof metrics>('metrics', (e) => {
      Object.assign(metrics, e.payload);
    });

    // BLE connection status
    await listen<{ trainer?: string; hrm?: string }>('ble', (e) => {
      if (e.payload.trainer) devices.trainer = e.payload.trainer as typeof devices.trainer;
      if (e.payload.hrm) devices.hrm = e.payload.hrm as typeof devices.hrm;
    });
  });
</script>

{@render children()}
```

- [ ] **Step 2: Write minimal app.css**

Replace `src/app.css`:

```css
:root {
  --bg: #0A0A0F;
  --surface: #13131A;
  --power: #FF6B00;
  --hr: #FF3366;
  --target: #4488FF;
  --text: #E8E8F0;
  --muted: #6B6B80;
  --font-mono: 'Courier New', 'Consolas', monospace;
}

* { box-sizing: border-box; margin: 0; padding: 0; }

body {
  background: var(--bg);
  color: var(--text);
  font-family: sans-serif;
  height: 100vh;
}
```

- [ ] **Step 3: Verify**

```powershell
npm run check
```

Expected: no errors.

- [ ] **Step 4: Commit**

```powershell
git add src/routes/+layout.svelte src/app.css
git commit -m "feat: layout with Tauri event listeners and global styles"
```

---

## Task 10: Dashboard page — scan, connect, live metrics

**Files:**
- Modify: `src/routes/+page.svelte`

- [ ] **Step 1: Write +page.svelte**

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { devices } from '$lib/stores/devices.svelte';
  import { metrics } from '$lib/stores/session.svelte';

  type DeviceInfo = { id: string; name: string; kind: 'trainer' | 'hrm' | 'unknown' };

  let scanning = $state(false);
  let found: DeviceInfo[] = $state([]);
  let wattTarget = $state(100);
  let ergActive = $state(false);

  async function scan() {
    scanning = true;
    found = [];
    try {
      found = await invoke<DeviceInfo[]>('scan_devices');
    } finally {
      scanning = false;
    }
  }

  async function connectTrainer(id: string) {
    devices.trainer = 'connecting';
    try {
      await invoke('connect_trainer', { deviceId: id });
    } catch (e) {
      devices.trainer = 'disconnected';
      console.error(e);
    }
  }

  async function connectHrm(id: string) {
    devices.hrm = 'connecting';
    try {
      await invoke('connect_hrm', { deviceId: id });
    } catch (e) {
      devices.hrm = 'disconnected';
      console.error(e);
    }
  }

  async function enableErg() {
    await invoke('request_control_and_start');
    ergActive = true;
  }

  async function sendWatts() {
    await invoke('set_target_power', { watts: wattTarget });
  }

  async function disconnect() {
    await invoke('disconnect_all');
    ergActive = false;
  }
</script>

<main>
  <header>
    <h1>OpenCycling</h1>
    <div class="device-status">
      <span class="status" class:connected={devices.trainer === 'connected'}>
        Trainer: {devices.trainer}
      </span>
      <span class="status" class:connected={devices.hrm === 'connected'}>
        HRM: {devices.hrm}
      </span>
    </div>
  </header>

  <section class="metrics">
    <div class="metric">
      <span class="value" style="color: var(--power)">{metrics.power}</span>
      <span class="label">W</span>
    </div>
    <div class="metric">
      <span class="value" style="color: var(--hr)">{metrics.hr}</span>
      <span class="label">bpm</span>
    </div>
    <div class="metric">
      <span class="value">{metrics.cadence}</span>
      <span class="label">rpm</span>
    </div>
  </section>

  <section class="controls">
    <button onclick={scan} disabled={scanning}>
      {scanning ? 'Scanning…' : 'Scan BLE'}
    </button>
    <button onclick={disconnect}>Disconnect all</button>

    {#if found.length > 0}
      <ul class="device-list">
        {#each found as d}
          <li>
            <span>{d.name} ({d.kind})</span>
            {#if d.kind === 'trainer'}
              <button onclick={() => connectTrainer(d.id)}>Connect trainer</button>
            {:else if d.kind === 'hrm'}
              <button onclick={() => connectHrm(d.id)}>Connect HRM</button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  {#if devices.trainer === 'connected'}
    <section class="erg">
      {#if !ergActive}
        <button onclick={enableErg}>Enable ERG control</button>
      {:else}
        <label>
          Target power: <strong>{wattTarget}W</strong>
          <input type="range" min="50" max="600" bind:value={wattTarget} />
        </label>
        <button onclick={sendWatts}>Set {wattTarget}W</button>
      {/if}
    </section>
  {/if}
</main>

<style>
  main { padding: 2rem; max-width: 800px; margin: 0 auto; }
  header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 2rem; }
  h1 { font-size: 1.5rem; letter-spacing: 0.1em; text-transform: uppercase; }
  .device-status { display: flex; gap: 1rem; }
  .status { font-size: 0.85rem; color: var(--muted); }
  .status.connected { color: #22c55e; }

  .metrics { display: flex; gap: 3rem; margin-bottom: 2rem; }
  .metric { display: flex; flex-direction: column; align-items: center; }
  .value { font-family: var(--font-mono); font-size: 4rem; font-weight: bold; line-height: 1; }
  .label { font-size: 0.85rem; color: var(--muted); text-transform: uppercase; }

  .controls { display: flex; flex-direction: column; gap: 1rem; }
  button {
    background: var(--surface); color: var(--text); border: 1px solid #2a2a3a;
    padding: 0.5rem 1rem; cursor: pointer; font-size: 0.9rem;
  }
  button:hover { border-color: var(--power); }
  button:disabled { opacity: 0.5; cursor: not-allowed; }

  .device-list { list-style: none; display: flex; flex-direction: column; gap: 0.5rem; }
  .device-list li { display: flex; justify-content: space-between; align-items: center;
    padding: 0.5rem; background: var(--surface); }

  .erg { margin-top: 2rem; display: flex; flex-direction: column; gap: 1rem; }
  input[type=range] { width: 300px; accent-color: var(--power); }
</style>
```

- [ ] **Step 2: Run the app**

```powershell
npm run tauri dev
```

Expected: window opens with dashboard. Scan should find BLE devices after ~5 seconds. Trainer and HRM connect buttons appear for FTMS/HRS devices. Metrics update in real-time from the trainer.

- [ ] **Step 3: Manual test checklist**
  - [ ] Scan returns D500 as "trainer", Polar H10 as "hrm"
  - [ ] Connect trainer → status shows "connected"
  - [ ] Connect HRM → status shows "connected"
  - [ ] Power (W) updates live while pedaling
  - [ ] Cadence (rpm) updates live
  - [ ] HR (bpm) updates live from Polar H10
  - [ ] "Enable ERG control" sends Request Control + Start without error
  - [ ] Watt slider + "Set Xw" button changes trainer resistance (verify by feel)
  - [ ] "Disconnect all" resets statuses

- [ ] **Step 4: Commit**

```powershell
git add src/routes/+page.svelte
git commit -m "feat: dashboard with BLE scan, connect, live metrics, ERG slider"
```

---

## Task 11: ERG keep-alive + auto-reconnect

**Files:**
- Modify: `src-tauri/src/ble/mod.rs`

- [ ] **Step 1: Add keep-alive task to BleActor::connect_trainer**

In `connect_trainer`, after `self.trainer = Some(peripheral)`, spawn a keep-alive task. Add these fields to `BleActor` struct first:

```rust
// Add to BleActor struct:
last_target_watts: u16,
keepalive_tx: Option<tokio::sync::watch::Sender<u16>>,
```

Then add this to `BleActor::new` initialization (update `spawn`):

```rust
// Replace the BleActor instantiation in spawn():
let actor = BleActor {
    rx,
    adapter: None,
    trainer: None,
    hrm: None,
    app_handle,
    last_target_watts: 0,
    keepalive_tx: None,
};
```

At the end of `connect_trainer`, before `self.trainer = Some(peripheral)`:

```rust
// Keep-alive: re-send target power every 10s
let (ka_tx, mut ka_rx) = tokio::sync::watch::channel(0u16);
self.keepalive_tx = Some(ka_tx);

let trainer_clone = peripheral.clone();
let cp_uuid = CONTROL_POINT_UUID.to_string();

tokio::spawn(async move {
    let mut interval = time::interval(Duration::from_secs(10));
    interval.tick().await; // skip first immediate tick
    loop {
        interval.tick().await;
        let watts = *ka_rx.borrow();
        if watts == 0 { continue; }
        let chars = trainer_clone.characteristics();
        let Some(cp) = chars.iter()
            .find(|c| c.uuid.to_string().to_lowercase() == cp_uuid)
            .cloned()
        else { break };
        let cmd = build_set_target_power_cmd(watts);
        let _ = trainer_clone.write(&cp, &cmd, WriteType::WithResponse).await;
        log::debug!("ERG keep-alive: {watts}W");
    }
});
```

Update `set_target_power` to also update the keep-alive channel:

```rust
async fn set_target_power(&self, watts: u16) {
    // ... existing write code ...

    // Update keep-alive
    if let Some(tx) = &self.keepalive_tx {
        let _ = tx.send(watts);
    }
}
```

- [ ] **Step 2: Add auto-reconnect on disconnect**

In the `connect_trainer` notification listener spawn, wrap the notification loop with disconnect detection:

```rust
tokio::spawn(async move {
    use futures::StreamExt;
    while let Some(notif) = notifications.next().await {
        if notif.uuid == ibd_uuid {
            if let Some(data) = parse_indoor_bike_data(&notif.value) {
                let _ = app_handle.emit("metrics_ble", serde_json::json!({
                    "power": data.power_w.max(0) as u32,
                    "cadence": data.cadence_rpm as u32,
                }));
            }
        }
    }
    // Stream ended = disconnected
    log::warn!("Trainer notification stream ended — disconnected");
    let _ = app_handle.emit("ble", serde_json::json!({ "trainer": "disconnected" }));
});
```

Auto-reconnect from the actor's run loop is complex in this architecture (requires storing the device ID). For Phase 1/2, emit the disconnect event so the user can manually reconnect. Full auto-reconnect is a Phase 2 polish item — defer.

- [ ] **Step 3: Verify compile**

```powershell
cd src-tauri && cargo build && cd ..
```

- [ ] **Step 4: Commit**

```powershell
git add src-tauri/src/ble/mod.rs
git commit -m "feat: ERG keep-alive every 10s + disconnect detection"
```

---

## Self-Review

**Spec coverage check:**

| Spec requirement | Covered in |
|---|---|
| Tauri v2 + SvelteKit scaffold | Task 1, 2 |
| BleActor with scan/connect | Task 6 |
| FTMS Indoor Bike Data read | Task 4, 6 |
| HRS Polar H10 read | Task 5, 6 |
| FTMS Control Point sequence (0x00, 0x07) | Task 6 |
| ERG Set Target Power (0x05) | Task 4, 6 |
| ERG write on change (not every second) | Task 6 — `set_target_power` is user-triggered via command |
| Keep-alive every 10s | Task 11 |
| Auto-reconnect with backoff | Partially — disconnect detection done, full backoff deferred to Phase 3 |
| Tauri events: `ble`, `metrics_ble` | Task 6, 9 |
| Svelte 5 runes (not legacy stores) | Task 8 |
| English UI | Task 10 |
| Manual watt slider (Phase 2 test) | Task 10 |
| `scan_devices`, `connect_trainer`, `connect_hrm`, `disconnect_all` commands | Task 7 |
| `set_target_power`, `request_control_and_start` commands | Task 7 |

**Gaps / notes:**
- Full auto-reconnect with exponential backoff: deferred. After Phase 3, add a retry loop in `BleActor::run` when disconnect is detected.
- `btleplug` on Windows: on some Windows versions, `peripheral.clone()` may not be available or may not share the underlying connection. If the keep-alive spawn fails to write, this is the likely cause — the connection context belongs to the original Peripheral instance. In that case, move the keep-alive into the notification listener spawn instead.
- The FTMS flag parsing in Task 4 must be validated against the D500's actual output. Use nRF Connect to capture a raw Indoor Bike Data notification and verify the flag bytes before trusting the parser.

---

*Next plan: Phase 3 — Workout Engine (.zwo parser + SessionActor tick)*
