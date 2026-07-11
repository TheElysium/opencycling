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
- `export/tcx.rs` — `build_tcx(&SessionDetail) -> String` builds a Garmin TCX activity (Sport="Biking") from a recorded session: one `<Trackpoint>` per 1 Hz sample (time, HR, cadence, power via the `ns3:` ActivityExtension namespace) plus a `<Notes>` block describing the workout structure (warmup / intervals / cooldown). Free text is XML-escaped. Used by the `export_session_tcx` command (local `.tcx` file).

`metrics.rs` — the single Rust zone module: `WorkoutType` enum, `ZONE_THRESHOLDS` / `zone_of` / `zone_name` / `fallback_label` (block labeling), `classify` (workout-type heuristic), `normalized_power` / `derive_metrics` (NP/IF/TSS). The TS `WorkoutType` type is generated into `src/lib/bindings.ts`; the only remaining hand-kept mirrors are `classify` and `zoneOf`/`ZONE_THRESHOLDS` in `src/lib/metrics.ts` (used for planned-workout preview metrics and display coloring) — keep their boundaries and heuristics identical to `metrics.rs`.

**Aero position detection** — all scoring lives in the webview (see Frontend below); Rust stays "dumb" and only stores already-projected numbers. The `report_aero` command feeds `SessionActor.last_aero: Option<bool>` (the frontend's smoothed + debounced aero/upright decision), which is written into the next 1 Hz sample as `aero_score` and reset to `None` on `Start`. On finalize, `DbActor` computes `sessions.aero_pct` as `AVG(aero_score)` over the session's samples. Settings carry a global default `aero_enabled` flag. Schema added in `db/migrations.rs` (v4 → v5: `session_metrics.aero_score`, `sessions.aero_pct`, `settings.aero_enabled`).

**Tokio actors** — communicate exclusively via `mpsc` channels. Each lives in a module with `actor.rs` (loop/logic), `command.rs` (the `*Handle` public API), and `types.rs`:
- `ble/` (`BleActorHandle`) — BLE scan/connect, ERG keep-alive (retransmit last target every 10s), emits `ble_metrics` every second; emits `ble_error` and `ble_disconnected` on failures.
- `session/` (`SessionActorHandle`) — session state machine, ticks every second, emits `session_metrics`. State is a `State` trait with variant structs in `state.rs` (WaitingForRider → Running → Paused → Finished). Consumes `ble_metrics`, drives ERG targets, persists samples via the DB actor.
- `db/` (`DbActorHandle`) — wraps SQLite (`migrations.rs` for schema), stores sessions/samples and the settings row.

Actors are wired together in `lib.rs::run()` (the `.setup()` closure) and registered with `app.manage(...)`.

**Error handling**: all errors flow through `AppError` (`errors.rs`, via `thiserror`). `AppError` implements `serde::Serialize` for Tauri command returns.

### Frontend (`src/`)

SvelteKit routes: `/` (connection), `/workouts`, `/workouts/detail`, `/session`, `/history`, `/history/[id]`, `/settings`. Sidebar hidden on `/session`.

Shared state lives in `.svelte.ts` rune stores: `lib/ble.svelte.ts`, `lib/session.svelte.ts`, `lib/workout.svelte.ts`, `lib/aero.svelte.ts`. Helpers: `lib/db.ts`, `lib/settings.ts`, `lib/format.ts`, `lib/metrics.ts`, `lib/audio.ts`, `lib/session-visuals.ts`, `lib/export.ts` (TCX export via the save dialog), `lib/aero.ts`.

Reusable components in `lib/components/` — e.g. `WorkoutChart` / `WorkoutPreview` / `WorkoutThumb` (block bars), `ZoneBar` (zone distribution), `SessionChart` (live power line), the session UI tiles (`MetricTile`, `MetricsStrip`, `PowerTile`, `CurrentBlockCard`, `SessionTimeline`, `SessionStatsPanel`, `SessionFinishedCard`, `SessionDetailRecap`, `BlocksList`), and the aero UI (`AeroCalibration`, `AeroPanel`).

**Aero position detection (`lib/aero.ts` + `lib/aero.svelte.ts`)** — detects, from a front-facing webcam, whether the rider holds an aero or upright position. `lib/aero.ts` is **pure** (unit-tested like `lib/metrics.ts`): COCO-17 keypoint indices, `extractFeatures` (upper-body only: `headDrop` / `earDrop` / `earSpread`, normalized by shoulder width), `buildCalibration` (per-rider upright→aero axis from two captured clusters via z-scoring), `scoreFrame` (projection to 0..1), plus `Smoother` (~1s sliding average) and `AeroGate` (hysteresis: enter > 0.55, exit < 0.45). `lib/aero.svelte.ts` is the rune store owning the MoveNet/TF.js detector, camera stream, hands-free calibration FSM (prep countdown → capture aero → capture upright → build + self-test), the live scoring loop, and the 1 Hz `report_aero` reporting. The TF.js model is **bundled offline** as a static asset (`/models/movenet-lightning/`), no CDN. The feature is frontend-gated: when the start checkbox is off, the camera never starts and `aero_score`/`aero_pct` stay `NULL`.

### Tauri bridge

The bridge is **typed end to end** via [tauri-specta]: every struct/enum crossing it derives `specta::Type`, commands are declared in `lib.rs::specta_builder()` (`collect_commands!`), and `src/lib/bindings.ts` is **generated** from them (committed; never edit by hand). The frontend calls commands through the generated `commands` object (`import { commands } from '$lib/bindings'`, e.g. `commands.getSession(id)`) instead of stringly `invoke()`. Regeneration: automatic on every debug startup (`pnpm tauri dev`), or one-shot via `cargo run --bin export_bindings` (from `src-tauri/`). Notable command: `flatten_workout_cmd(workout, ftp_w) -> Vec<FlatBlock>` exposes the Rust `flatten_workout` so the frontend never duplicates flatten/label logic. Rust pushes data via events consumed with plain `listen()` calls: `ble_metrics`, `session_metrics`, `ble_error`, `ble_reconnect`, `ble_disconnected`; their payload types are exported in `bindings.ts` too (`.typ::<T>()` in the builder). `docs/prd.md` is outdated; the bindings file is the contract.

## Key constraints

- **BLE device filtering by name prefix** — UUID-based filtering is unreliable on Windows WinRT. Filter by `"D500"` (trainer) and `"Polar"` (HRM).
- **Tests live in the same file** as the code they test (`#[cfg(test)]` module). No separate test files.
- **Actors are not unit-tested** — BleActor (BLE hardware), DbActor (SQLite I/O), and SessionActor (Tokio runtime) are validated manually or via integration tests only.
- **All project documentation** (issues, PRD, specs) is written in English. Conversation with the author is in French.