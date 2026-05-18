# OpenCycling — Design Spec

**Date:** 2026-05-18
**Status:** Approved

---

## Overview

Open-source desktop application for connected indoor trainers, focused on interval training. No multiplayer, no landscape video. Target: a clear, reliable, modern solo training experience.

**Out of scope (v1):** FIT export, Strava/Garmin API, FTP ramp test, Picture-in-Picture mode.

---

## Stack

| Layer | Technology |
|---|---|
| Desktop framework | Tauri v2 |
| Backend | Rust |
| BLE | btleplug (winrt feature on Windows) |
| Local database | rusqlite (SQLite) |
| Frontend | Svelte 5 + TypeScript (runes, not legacy stores) |
| Routing | SvelteKit with adapter-static, ssr = false |
| Charts | uPlot (lightweight, real-time) |
| Styles | CSS custom properties, no framework |

**Primary OS:** Windows. Cross-platform via Tauri.

---

## Hardware

| Device | Protocol | BLE Profile |
|---|---|---|
| Van Rysel D500 (trainer) | Bluetooth FTMS | Fitness Machine Service `0x1826` |
| Polar H10 (HRM) | Bluetooth HRS | Heart Rate Service `0x180D` |

---

## Architecture — Rust Actors

Three independent actors, each in its own tokio task, communicating via `mpsc` channels. No shared global mutex.

```
┌─────────────────────────────────────────────────────┐
│  BleActor                                           │
│  - Scan / connect / disconnect                      │
│  - FTMS notify → emits BleEvent (power, cadence)   │
│  - HRS notify → emits BleEvent (hr)                │
│  - Receives BleCommand (set target power)           │
│  - Auto-reconnect with exponential backoff          │
└────────────────────┬────────────────────────────────┘
                     │ BleEvent
                     ▼
┌─────────────────────────────────────────────────────┐
│  SessionActor                                       │
│  - Receives BLE metrics                             │
│  - Advances segments, manages timing (1s tick)      │
│  - Decides ERG target → sends BleCommand            │
│  - Writes 1 row/second to DB via DbActor            │
│  - Emits Tauri events (metrics, block, alert)       │
└────────────────────┬────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────┐
│  DbActor                                            │
│  - All SQLite operations serialized in a queue      │
│  - Handles session writes, history queries          │
│  - Handles settings reads/writes                    │
└─────────────────────────────────────────────────────┘
```

Each actor exposes a `Handle` struct (wrapping an `mpsc::Sender`). Tauri commands delegate to the relevant Handle — no direct state access from command handlers.

---

## BLE Module

### Device identification

Devices are identified by service UUID, not by name:
- D500: presence of FTMS service `0x1826`
- Polar H10: presence of HRS service `0x180D`

The user confirms selection in the UI after scan.

### Windows scan (btleplug WinRT)

Scan runs for 5 seconds, collecting devices continuously via a stream. Frontend receives devices progressively (no blocking wait). Feature flag required in `Cargo.toml`: `btleplug = { features = ["winrt"] }`.

### FTMS Control Point sequence

Required before any ERG command. Must be executed once at connection:

```
1. Write 0x00 (Request Control)  → wait for Indication: success
2. Write 0x07 (Start/Resume)     → wait for Indication: success
3. Write 0x05 + watts LE int16   → wait for Indication: success
```

Without steps 1 and 2, Set Target Power commands are silently ignored.

### ERG write strategy

- **Steady segments** (SteadyState, Recovery, IntervalsT ON/OFF): write Set Target Power only on segment change
- **Ramp segments** (Warmup, Cooldown): write Set Target Power every second (target changes each tick via linear interpolation)
- Keep-alive: re-send current target every 10s for steady segments (some trainers drop ERG without recent command)
- Whether keep-alive is needed is detected at connection via `Fitness Machine Feature (0x2ACC)`
- `FreeRide` segments: no ERG command is sent; trainer stays in free resistance mode
- Countdown alert (`remaining_s == 10`) is only emitted if block duration > 10s

### Auto-reconnect

On BLE disconnection, `BleActor` retries with exponential backoff: 1s → 2s → 4s → 8s → max 30s. A `"ble"` Tauri event with status `"connecting"` is emitted on each attempt.

### FTMS characteristics

| Characteristic | UUID | Operation |
|---|---|---|
| Fitness Machine Feature | `0x2ACC` | Read |
| Indoor Bike Data | `0x2AD2` | Notify (power, cadence, speed) |
| Supported Power Range | `0x2AD8` | Read |
| Fitness Machine Control Point | `0x2AD9` | Write + Indicate |
| Fitness Machine Status | `0x2ADA` | Notify |

**ERG command format:**
```
[0x05, watts_low_byte, watts_high_byte]  (little-endian signed int16)
Example: 200W → [0x05, 0xC8, 0x00]
```

### HRS — Polar H10

Characteristic `0x2A37`, Notify. First byte = flags. If flag bit0 = 0: BPM is byte 1 (u8). If flag bit0 = 1: BPM is bytes 1-2 (u16 LE).

---

## Workout Engine

### .zwo normalization

On load, the .zwo XML is parsed and normalized into a flat list of `Segment`. The engine never needs to know about .zwo block types at runtime.

```rust
struct Segment {
    duration_s: u32,
    power_target: PowerTarget,
    label: String,        // e.g. "Interval 3/5 — ON", "Warmup"
    index: u32,
    total: u32,
}

enum PowerTarget {
    Watts(u32),   // pre-computed: ratio × FTP at load time
    FreeRide,
}
```

`IntervalsT { Repeat: 5, OnDuration: 60, OffDuration: 60, OnPower: 1.2, OffPower: 0.5 }` → 10 consecutive segments.

FTP multiplication happens **at parse time**, not at each tick. A mid-session FTP change does not affect the ongoing session.

### Session tick (1s interval)

```
tokio::time::interval(Duration::from_secs(1)) fires
  │
  ├── elapsed_in_segment += 1
  ├── If elapsed_in_segment >= segment.duration_s
  │     ├── advance to next segment
  │     └── send BleCommand::SetTargetPower (or nothing if FreeRide)
  ├── Emit Tauri event "metrics"
  ├── Emit Tauri event "block" (if segment changed)
  ├── Emit Tauri event "alert" (if remaining_s == 10 or == 0)
  └── Send DbCommand::WriteDataPoint
```

### Supported .zwo block types

| Type | Notes |
|---|---|
| `Warmup` | Linear ramp low→high, interpolated per second |
| `Cooldown` | Linear ramp high→low, interpolated per second |
| `SteadyState` | Fixed power |
| `IntervalsT` | Expanded to N×(ON + OFF) segments |
| `Recovery` | Fixed power (low) |
| `FreeRide` | No ERG |

---

## Database Schema

```sql
CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
-- Keys: ftp (watts), weight (kg), name, workout_folder (absolute path)

CREATE TABLE sessions (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  started_at   INTEGER NOT NULL,   -- Unix timestamp
  duration_s   INTEGER NOT NULL,
  workout_name TEXT,
  workout_file TEXT,               -- filename only (not full path)
  tss          REAL,
  if_score     REAL,
  avg_power    REAL,
  avg_hr       INTEGER,
  avg_cadence  INTEGER,
  np           REAL
);

CREATE TABLE session_data (
  session_id INTEGER NOT NULL REFERENCES sessions(id),
  t          INTEGER NOT NULL,     -- seconds since session start
  power      INTEGER,              -- actual watts
  target     INTEGER,              -- target watts
  hr         INTEGER,              -- bpm
  cadence    INTEGER               -- rpm
);

CREATE INDEX idx_session_data ON session_data(session_id, t);
```

**Notes:**
- `workout_file` stores filename only; full path = `settings.workout_folder + filename`
- `session_data` produces ~3600 rows/hour — fine for SQLite with the index
- NP, TSS, IF are computed at session end and written to `sessions`

### NP / TSS / IF computation (end of session, Rust)

```
NP  = (mean of 30s rolling window values^4)^(1/4)
IF  = NP / FTP
TSS = (duration_s × NP × IF) / (FTP × 3600) × 100
```

NP rolling window: implemented as a ring buffer of 30 values in `SessionActor`.

---

## IPC — Tauri Commands

```rust
// BLE
#[tauri::command] scan_devices() -> Vec<Device>
#[tauri::command] connect_trainer(device_id: String) -> Result<()>
#[tauri::command] connect_hrm(device_id: String) -> Result<()>
#[tauri::command] disconnect_all() -> Result<()>

// Session
#[tauri::command] start_session(filename: String, ftp: u32) -> Result<()>
#[tauri::command] pause_session() -> Result<()>
#[tauri::command] resume_session() -> Result<()>
#[tauri::command] stop_session() -> Result<SessionSummary>

// Workout files
#[tauri::command] list_workouts() -> Vec<WorkoutMeta>       // scans workout_folder
#[tauri::command] load_workout(filename: String) -> Result<Workout>
#[tauri::command] save_workout(workout: Workout) -> Result<String>
#[tauri::command] import_workout(source_path: String) -> Result<String> // copies to workout_folder

// History
#[tauri::command] get_sessions(limit: u32, offset: u32) -> Vec<SessionSummary>
#[tauri::command] get_session_detail(id: i64) -> Result<SessionDetail>

// Settings
#[tauri::command] get_settings() -> Settings
#[tauri::command] save_settings(settings: Settings) -> Result<()>
```

## IPC — Tauri Events (Rust → Frontend)

```
"metrics" → {
  power: u32, target: u32, hr: u32, cadence: u32,
  elapsed_s: u32, remaining_s: u32, total_remaining_s: u32
}

"block" → {
  label: String, remaining_s: u32, power_target: u32,
  index: u32, total: u32
}

"ble" → {
  trainer: "connected" | "disconnected" | "connecting",
  hrm:     "connected" | "disconnected" | "connecting"
}

"alert" → {
  type: "countdown" | "block_change" | "session_start" | "session_end"
}
```

---

## Frontend

### Routes (SvelteKit, adapter-static, ssr = false)

```
src/routes/
  +page.svelte          # Dashboard / device connection
  session/+page.svelte  # Active session (priority screen)
  editor/+page.svelte   # Workout editor
  history/+page.svelte  # Session history
```

### State (Svelte 5 runes)

```typescript
// session.svelte.ts
let power = $state(0);
let target = $state(0);
let hr = $state(0);
let cadence = $state(0);
let elapsed_s = $state(0);
let remaining_s = $state(0);
let total_remaining_s = $state(0);

// devices.svelte.ts
let trainer_status = $state<"connected"|"disconnected"|"connecting">("disconnected");
let hrm_status = $state<"connected"|"disconnected"|"connecting">("disconnected");

// workout.svelte.ts
let segments = $state<Segment[]>([]);
let current_index = $state(0);
```

### Event listeners (single subscription point at app start)

```typescript
import { listen } from '@tauri-apps/api/event';

await listen('metrics', (e) => { /* update session runes */ });
await listen('block',   (e) => { /* update current_index */ });
await listen('ble',     (e) => { /* update device status */ });
await listen('alert',   (e) => { /* trigger Web Audio */ });
```

### Power zones (Coggan 7)

```typescript
const ZONES = [
  { name: 'Z1', max: 0.55,     color: '#6B7280' }, // Active Recovery
  { name: 'Z2', max: 0.75,     color: '#3B82F6' }, // Endurance
  { name: 'Z3', max: 0.90,     color: '#10B981' }, // Tempo
  { name: 'Z4', max: 1.05,     color: '#F59E0B' }, // Threshold
  { name: 'Z5', max: 1.20,     color: '#F97316' }, // VO2Max
  { name: 'Z6', max: 1.50,     color: '#EF4444' }, // Anaerobic
  { name: 'Z7', max: Infinity, color: '#7C3AED' }, // Neuromuscular
];

function getZone(ratio: number) {
  return ZONES.find(z => ratio < z.max) ?? ZONES[6];
}
```

### Session screen — displayed metrics

- Current power (W) — large, centered
- Target power (W)
- Heart rate (bpm)
- Cadence (rpm)
- Elapsed time / time remaining in current block
- Total session time remaining

### Workout timeline

- Horizontal visual of all segments, colored by Coggan zone
- Real-time progress cursor
- Next segment highlighted

### Real-time chart (uPlot)

- Target power (dashed line)
- Actual power (solid line)
- HR (secondary axis)
- 5-minute sliding window

### Audio alerts (Web Audio API)

- 3 short beeps: 10 seconds before block end
- 1 long beep: block change
- Low beep: session start / end

### UI Design

- Background: `#0A0A0F`
- Power accent: `#FF6B00`
- HR accent: `#FF3366`
- Target accent: `#4488FF`
- Monospace font for all numeric metrics
- Angular, sharp UI — no excessive border-radius or soft shadows
- Transitions: 150ms max

---

## .zwo Format

```xml
<workout_file>
  <author>string</author>
  <name>string</name>
  <description>string</description>
  <sportType>bike</sportType>
  <tags/>
  <workout>
    <Warmup     Duration="600"  PowerLow="0.40" PowerHigh="0.75"/>
    <SteadyState Duration="1200" Power="0.88"/>
    <IntervalsT  Repeat="5" OnDuration="60" OffDuration="60"
                 OnPower="1.20" OffPower="0.50"/>
    <Recovery   Duration="300"  Power="0.50"/>
    <Cooldown   Duration="600"  PowerLow="0.75" PowerHigh="0.40"/>
    <FreeRide   Duration="300"/>
  </workout>
</workout_file>
```

All power values are FTP ratios (0.88 = 88% FTP). Multiplied by user FTP at parse time.

---

## Development Phases

### Phase 1 — BLE Foundation
- [ ] Scaffold Tauri v2 + SvelteKit + Rust project (replace current bare Rust skeleton)
- [ ] BleActor: scan, connect D500 + Polar H10
- [ ] FTMS Indoor Bike Data read + HRS read
- [ ] Control Point sequence (Request Control → Start → Set Target Power)
- [ ] Basic real-time metrics display

### Phase 2 — ERG Control
- [ ] ERG write on block change + 10s keep-alive
- [ ] Manual test with watt slider in UI
- [ ] BLE auto-reconnect with exponential backoff

### Phase 3 — Workout Engine
- [ ] .zwo parser → flat Segment list
- [ ] SessionActor: 1s tick, segment advancement
- [ ] Tauri events: metrics, block, alert

### Phase 4 — Session UI
- [ ] Full session screen: metrics, timeline (Coggan 7 zones), real-time chart
- [ ] Audio alerts (Web Audio API)
- [ ] DbActor: session recording

### Phase 5 — Workout Library & Editor
- [ ] workout_folder setting (user-chosen path)
- [ ] Workout list + .zwo import
- [ ] Workout editor (drag-and-drop blocks, timeline preview, save as .zwo)

### Phase 6 — History
- [ ] Session history list (date, duration, TSS, IF, avg power)
- [ ] Session detail view with chart
- [ ] NP, TSS, IF computation at session end

### Phase 7 — Future (out of v1 scope)
- [ ] FIT export (Garmin / Strava manual upload)
- [ ] FTP ramp test
- [ ] Strava / Garmin Connect API

---

## Key Implementation Notes

1. **btleplug on Windows**: use `winrt` feature flag. BLE connections fail frequently — auto-reconnect is mandatory, not optional.
2. **Tauri v2 events**: use `app.emit()` for global events from async Rust tasks. Do not use deprecated v1 APIs.
3. **BLE thread**: btleplug is async (tokio). BleActor runs in background, never blocks the main thread.
4. **FTP at parse time**: all .zwo power ratios are converted to absolute watts when the workout is loaded, using the FTP from settings at that moment.
5. **NP ring buffer**: implement as a fixed-size circular buffer of 30 u32 values in SessionActor.
6. **FTMS handshake**: always send Request Control + Start before any Set Target Power. Without this, ERG commands are silently ignored.