# AGENTS.md

Guidance for any AI coding agent working in this repository.

## Commands

All Rust commands run from `src-tauri/`:

```bash
cargo fmt --all --check              # formatting check (CI-enforced)
cargo clippy --all-targets -- -D warnings   # lint (CI-enforced, zero warnings allowed)
cargo test                           # run all unit tests
cargo test test_name                 # run a single test by name (partial match)
cargo run --bin export_bindings      # regenerate src/lib/bindings.ts
```

Frontend and full app (from repo root):

```bash
pnpm tauri dev                      # run the full Tauri app (frontend + backend); regenerates bindings.ts on startup
pnpm check                          # TypeScript/Svelte type checking (svelte-check)
pnpm test                           # frontend unit tests (vitest)
```

**Before declaring a task done**, run: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `pnpm check`, `pnpm test`. All must pass.

## Architecture

OpenCycling is a **Tauri v2 desktop app**: SvelteKit 5 frontend (Svelte runes) + Rust backend.

### Rust backend (`src-tauri/src/`)

**Pure parsers** — no I/O, no BLE dependencies, operate on `&[u8]` slices or `&str`:
- `ble/ftms/` — parses FTMS `Indoor Bike Data` notifications (0x2AD2) and builds ERG commands. Split into `mod.rs` (parser logic), `types.rs` (structs, flags, `FeatureVal` enum), `features.rs` (per-field parse functions + FEATURES table).
- `ble/hrs.rs` — parses HRS `Heart Rate Measurement` notifications (0x2A37).
- `workout/zwo.rs` — parses `.zwo` Zwift XML into `ParsedWorkout` (`workout/types.rs`).

**Tokio actors** — communicate exclusively via `mpsc` channels, exposed as `*Handle` types:
- `ble/actor.rs` (`BleActorHandle`) — BLE scan/connect, ERG keep-alive (retransmit last target every 10s), emits `ble_metrics` every second.
- `session/actor.rs` (`SessionActorHandle`) — session state machine (`session/state.rs`: WaitingForRider → Running → Paused → Finished), ticks every second, emits `session_metrics`, persists samples via the DB actor.
- `db/actor.rs` (`DbActorHandle`) — wraps SQLite (`db/migrations.rs` for schema), stores sessions/samples and the settings row.

Actors are wired in `lib.rs::run()` (`.setup()` closure) and registered with `app.manage(...)`.

**Error handling**: all errors flow through `AppError` (`errors.rs`, via `thiserror`). `AppError` implements `serde::Serialize` (a bare string) for Tauri command returns.

### Frontend (`src/`)

SvelteKit routes: `/` (connection), `/workouts`, `/workouts/detail`, `/session`, `/history`, `/history/[id]`, `/settings`. Sidebar hidden on `/session`.

Shared state lives in `.svelte.ts` rune stores: `lib/ble.svelte.ts`, `lib/session.svelte.ts`, `lib/workout.svelte.ts`, `lib/aero.svelte.ts`. Helpers: `lib/db.ts`, `lib/settings.ts`, `lib/format.ts`, `lib/metrics.ts`, `lib/ftp.ts`, `lib/audio.ts`, `lib/export.ts` (TCX via save dialog), `lib/aero.ts` (pure, unit-tested webcam aero-position scoring; `lib/aero.svelte.ts` owns the MoveNet detector, bundled offline under `static/models/`).

Reusable components in `lib/components/`: `WorkoutChart` / `WorkoutPreview` / `WorkoutThumb` (block bars), `ZoneBar`, `SessionChart`, session UI tiles (`MetricTile`, `MetricsStrip`, `PowerTile`, `CurrentBlockCard`, `SessionTimeline`, `SessionStatsPanel`, `SessionFinishedCard`, `SessionDetailRecap`, `BlocksList`), aero UI (`AeroCalibration`, `AeroPanel`).

### Tauri bridge

The bridge is **typed end to end** via tauri-specta: every struct/enum crossing it derives `specta::Type`, commands are declared in `lib.rs::specta_builder()` (`collect_commands!`), and `src/lib/bindings.ts` is **generated** from them (committed; never edit by hand). The frontend calls commands through the generated `commands` object (`import { commands } from '$lib/bindings'`), never stringly `invoke()`. Rust pushes events consumed with plain `listen()`: `ble_metrics`, `session_metrics`, `ble_error`, `ble_reconnect`, `ble_disconnected`; payload types are exported in `bindings.ts` too. CI regenerates the bindings and fails on drift. `docs/prd.md` is outdated; `bindings.ts` is the contract.

Do not duplicate Rust logic in TypeScript: flatten/labeling lives in Rust (`flatten_workout`), exposed via `flatten_workout_cmd`. Known accepted mirrors: `classify`/`zoneOf` in `lib/metrics.ts` (documented next to the Rust originals).

## Code style rules

- **Comments: 2 lines max.** Write the *why* (a constraint, a link to the spec, a mirror warning), never the *what*. If a comment is needed to explain *what* the code does, restructure the code instead. Mirror comments must name the mirrored source (file + symbol).
- **Cyclomatic complexity: keep functions small.** Aim for ≤ ~10 branches per function; prefer `match` and lookup tables over `if/else` chains, and extract a helper when nesting grows or a function stops fitting on one screen. Pure parsers stay branch-per-protocol-field — that is their shape; factor shared patterns into the FEATURES table rather than adding ad-hoc branches.
- **Lint discipline: zero warnings.** Clippy runs with `-D warnings` in CI, `svelte-check` must report 0 errors *and* 0 warnings. Do not silence a lint with `#[allow]`/`// eslint-disable`-style pragmas without a one-line justification comment.
- **Tests pin behavior, not implementation.** No tautological tests (asserting a constant equals itself, re-stating the code). Pin boundaries, error cases, and wire formats; a refactor should not require rewriting passing tests.

## Key constraints

- **BLE device filtering by name prefix** — UUID-based filtering is unreliable on Windows WinRT. Filter by `"D500"` (trainer) and `"Polar"` (HRM).
- **Tests live in the same file** as the code they test (`#[cfg(test)]` module). No separate test files.
- **Actors are not unit-tested** — BleActor (BLE hardware), DbActor (SQLite I/O), and SessionActor (Tokio runtime) are validated manually or via integration tests only.
- **All project documentation** (issues, PRD, specs) is written in English. Conversation with the author is in French.
