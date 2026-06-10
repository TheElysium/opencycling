# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

All Rust commands run from `src-tauri/`:

```bash
cargo test                          # run all unit tests
cargo test test_name                # run a single test by name (partial match)
cargo clippy                        # lint
```

Frontend and full app (from repo root):

```bash
pnpm tauri dev                      # run the full Tauri app (frontend + backend)
pnpm check                          # TypeScript/Svelte type checking
```

## Architecture

OpenCycling is a **Tauri v2 desktop app**: SvelteKit 5 frontend (Svelte runes) + Rust backend.

### Rust backend (`src-tauri/src/`)

**Pure parsers** — no I/O, no BLE dependencies, operate on `&[u8]` slices or `&str`:
- `ble/ftms/` — parses FTMS `Indoor Bike Data` notifications (0x2AD2) and builds ERG commands. Split into `mod.rs` (parser logic), `types.rs` (structs, flags, `FeatureVal` enum), `features.rs` (per-field parse functions + FEATURES table).
- `ble/hrs.rs` — parses HRS `Heart Rate Measurement` notifications (0x2A37).
- `workout/zwo.rs` — parses `.zwo` Zwift XML files into `ParsedWorkout` (`WorkoutBlock` list). `workout/library.rs` lists/parses every `.zwo` in a folder; `workout/types.rs` holds `ParsedWorkout`, `WorkoutBlock`, `SportType`.

`metrics.rs` — shared `WorkoutType` enum (zone classification); **must mirror** the TypeScript `WorkoutType` in `src/lib/metrics.ts`.

**Tokio actors** — communicate exclusively via `mpsc` channels. Each lives in a module with `actor.rs` (loop/logic), `command.rs` (the `*Handle` public API), and `types.rs`:
- `ble/` (`BleActorHandle`) — BLE scan/connect, ERG keep-alive (retransmit last target every 10s), emits `ble_metrics` every second; emits `ble_error` and `ble_disconnected` on failures.
- `session/` (`SessionActorHandle`) — session state machine, ticks every second, emits `session_metrics`. State is a `State` trait with variant structs in `state.rs` (WaitingForRider → Running → Paused → Finished). Consumes `ble_metrics`, drives ERG targets, persists samples via the DB actor.
- `db/` (`DbActorHandle`) — wraps SQLite (`migrations.rs` for schema), stores sessions/samples and the settings row.

Actors are wired together in `lib.rs::run()` (the `.setup()` closure) and registered with `app.manage(...)`.

**Error handling**: all errors flow through `AppError` (`errors.rs`, via `thiserror`). `AppError` implements `serde::Serialize` for Tauri command returns.

### Frontend (`src/`)

SvelteKit routes: `/` (connection), `/workouts`, `/workouts/detail`, `/session`, `/history`, `/history/[id]`, `/settings`. Sidebar hidden on `/session`.

Shared state lives in `.svelte.ts` rune stores: `lib/ble.svelte.ts`, `lib/session.svelte.ts`, `lib/workout.svelte.ts`. Helpers: `lib/db.ts`, `lib/settings.ts`, `lib/format.ts`, `lib/metrics.ts`, `lib/audio.ts`, `lib/session-visuals.ts`.

Reusable components in `lib/components/` — e.g. `WorkoutChart` / `WorkoutPreview` / `WorkoutThumb` (block bars), `ZoneBar` (zone distribution), `SessionChart` (live power line), and the session UI tiles (`MetricTile`, `MetricsStrip`, `PowerTile`, `CurrentBlockCard`, `SessionTimeline`, `SessionStatsPanel`, `SessionFinishedCard`, `SessionDetailRecap`, `BlocksList`).

### Tauri bridge

Frontend calls Rust via `invoke('<command>', args)`. Commands are registered in `lib.rs`: `load_workout`, `list_workouts_cmd`, `scan_devices`, `connect_trainer`, `connect_hrm`, `set_target_power`, `get_settings`, `update_settings`, `start_session`, `pause_session`, `resume_session`, `stop_session`, `skip_block`, `get_session_snapshot`, `list_sessions`, `get_session`, `delete_session`. Rust pushes data via events: `ble_metrics`, `session_metrics`, `ble_error`, `ble_disconnected`. Full contracts are documented in `docs/prd.md`.

## Key constraints

- **BLE device filtering by name prefix** — UUID-based filtering is unreliable on Windows WinRT. Filter by `"D500"` (trainer) and `"Polar"` (HRM).
- **Tests live in the same file** as the code they test (`#[cfg(test)]` module). No separate test files.
- **Actors are not unit-tested** — BleActor (BLE hardware), DbActor (SQLite I/O), and SessionActor (Tokio runtime) are validated manually or via integration tests only.
- **All project documentation** (issues, PRD, specs) is written in English. Conversation with the author is in French.