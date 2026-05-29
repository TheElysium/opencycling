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

**Pure parsers** — no I/O, no BLE dependencies, operate on `&[u8]` slices:
- `ble/ftms/` — parses FTMS `Indoor Bike Data` notifications (0x2AD2) and builds ERG commands. Split into `mod.rs` (parser logic), `types.rs` (structs, flags, `FeatureVal` enum), `features.rs` (per-field parse functions + FEATURES table).
- `ble/hrs.rs` — parses HRS `Heart Rate Measurement` notifications (0x2A37).
- `ble/zwo/` (future) — parses `.zwo` Zwift XML files into `WorkoutBlock` lists.

**Tokio actors** (future phases) — communicate exclusively via `mpsc` channels:
- `BleActor` — BLE scan/connect, ERG keep-alive (retransmit last target every 10s), emits `ble_metrics` event every second.
- `SessionActor` — session state machine, ticks every second, emits `session_metrics`.
- `DbActor` — wraps SQLite, stores sessions and samples.

**Error handling**: all errors flow through `AppError` (`errors.rs`, via `thiserror`). `AppError` implements `serde::Serialize` for Tauri command returns.

### Frontend (`src/`)

SvelteKit routes: `/` (connection), `/workouts`, `/workouts/[slug]`, `/session`, `/history`, `/history/[id]`, `/settings`. Sidebar hidden on `/session`.

Reusable SVG components: `WorkoutChart` (block bars), `PowerChart` (line chart), `ZoneBar` (zone distribution).

### Tauri bridge

Frontend calls Rust via `invoke('<command>', args)`. Rust pushes data via events (`ble_metrics`, `session_metrics`, `ble_error`). Full command and event contracts are documented in `docs/prd.md`.

## Key constraints

- **BLE device filtering by name prefix** — UUID-based filtering is unreliable on Windows WinRT. Filter by `"D500"` (trainer) and `"Polar"` (HRM).
- **Tests live in the same file** as the code they test (`#[cfg(test)]` module). No separate test files.
- **Actors are not unit-tested** — BleActor (BLE hardware), DbActor (SQLite I/O), and SessionActor (Tokio runtime) are validated manually or via integration tests only.
- **All project documentation** (issues, PRD, specs) is written in English. Conversation with the author is in French.