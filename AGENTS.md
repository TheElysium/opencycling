# AGENTS.md

Guidance for any AI coding agent working in this repository.

## Commands

All Rust commands run from `src-tauri/`:

```bash
cargo fmt --all --check              # formatting check (CI-enforced)
cargo clippy --all-targets -- -D warnings   # lint (CI-enforced, zero warnings allowed)
cargo test                           # run all unit tests
cargo test test_name                 # run a single test by name (partial match)
cargo audit                          # dependency security audit (RustSec, CI-enforced)
cargo run --bin export_bindings      # regenerate src/lib/bindings.ts
```

Quality gates (CI-enforced, local via `scripts/gate.sh`):

```bash
bash scripts/gate.sh                 # full gate: fmt + clippy + tests + audit + size + frontend
bash scripts/check_size.sh           # file size gate only: no source file over 1000 lines
```

- `scripts/check_size.sh` fails if any source file (`.rs`, `.ts`, `.js`, `.svelte`, excluding the generated `bindings.ts`) exceeds 1000 lines — this is the anti-megafile gate for agent-generated code.
- Cyclomatic complexity is gated natively by clippy: `cognitive_complexity = "deny"` in `Cargo.toml` with threshold `cognitive-complexity-threshold = 15` in `src-tauri/clippy.toml`. Existing actor loops are baselined with `#[expect(clippy::cognitive_complexity)]` + a why comment; `#[expect]` errors once refactored, so the baseline self-removes. Do not add new `#[expect]` without a justification comment.
- Frontend complexity is gated the same way by ESLint (`complexity: error 15` in `eslint.config.js`, threshold mirrors clippy.toml). Existing offenders get `/* eslint-disable-next-line complexity */` + a why comment only with justification.

Frontend and full app (from repo root):

```bash
pnpm tauri dev                      # run the full Tauri app (frontend + backend); regenerates bindings.ts on startup
pnpm check                          # TypeScript/Svelte type checking (svelte-check)
pnpm lint                           # ESLint (0 errors and 0 warnings enforced, CI too)
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

**Tokio actors** — communicate exclusively via `mpsc` channels. Each actor module follows the same split: `command.rs` holds the `*Handle` (the channel endpoint Tauri commands delegate to) and the `*Command` enum; `actor.rs`/`types.rs` hold the actor loop and shared types:
- `ble/` (`BleActorHandle` in `command.rs`) — BLE scan/connect, ERG keep-alive (retransmit last target every 10s), emits `ble_metrics` every second. `sim.rs` is a full simulator standing in for the real actor when env var `OPENYCLING_SIM=1` is set (synthetic 1Hz power/HR/cadence, drop/restore scenarios; UI in `SimPanel.svelte`).
- `session/` (`SessionActorHandle` in `command.rs`) — session state machine (`session/state.rs`: WaitingForRider → Running/Ramping → Paused → Finished), ticks every second, emits `session_metrics`, persists samples via the DB actor.
- `db/` (`DbActorHandle` in `command.rs`) — wraps SQLite (`db/migrations.rs` for schema), stores sessions/samples, settings row and Strava auth.

Actors are wired in `lib.rs::run()` (`.setup()` closure) and registered with `app.manage(...)`.

**Derived metrics** — `metrics.rs` (pure): Normalized Power (Coggan 30s rolling), `derive_metrics()` (NP/IF/TSS), `classify()`/`zone_of()`; mirrors `classify`/`zoneOf` in `lib/metrics.ts`.

**Export & Strava** — `export/tcx.rs` (pure) builds the TCX file and the Strava activity description; `strava/oauth.rs` runs the OAuth code flow via a local loopback listener (`127.0.0.1:8123`) with client ID from a proxy; `strava/api.rs` uploads TCX (multipart, then polls the upload). Frontend never builds TCX.

**Error handling**: all errors flow through `AppError` (`errors.rs`, via `thiserror`). `AppError` implements `serde::Serialize` (a bare string) for Tauri command returns.

### Frontend (`src/`)

SvelteKit routes: `/` (connection), `/workouts`, `/workouts/detail`, `/session`, `/history`, `/history/[id]`, `/settings`. Sidebar hidden on `/session`.

Shared state lives in `.svelte.ts` rune stores: `lib/ble.svelte.ts`, `lib/session.svelte.ts`, `lib/workout.svelte.ts`, `lib/aero.svelte.ts`. Helpers: `lib/db.ts`, `lib/settings.ts`, `lib/format.ts`, `lib/metrics.ts`, `lib/ftp.ts`, `lib/audio.ts`, `lib/devices.ts` (auto-connect matching), `lib/chart-scale.ts`, `lib/session-visuals.ts`, `lib/strava.ts` (thin wrappers over generated commands), `lib/updater.ts` (`@tauri-apps/plugin-updater`, no-op under `tauri dev`), `lib/export.ts` (save dialog, then delegates to the Rust `export_session_tcx` command), `lib/aero.ts` (pure, unit-tested webcam aero-position scoring; `lib/aero.svelte.ts` owns the MoveNet detector, bundled offline under `static/models/`).

Reusable components in `lib/components/`: `WorkoutChart` / `WorkoutPreview` / `WorkoutThumb` (block bars), `ZoneBar`, `SessionChart`, session UI tiles (`MetricTile`, `MetricsStrip`, `PowerTile`, `CurrentBlockCard`, `SessionTimeline`, `SessionStatsPanel`, `SessionFinishedCard`, `SessionDetailRecap`, `BlocksList`, `FtpTestResult`), aero UI (`AeroCalibration`, `AeroPanel`), sim UI (`SimPanel`, hidden when sim mode is off).

### Tauri bridge

The bridge is **typed end to end** via tauri-specta: every struct/enum crossing it derives `specta::Type`, commands are declared in `lib.rs::specta_builder()` (`collect_commands!`), and `src/lib/bindings.ts` is **generated** from them (committed; never edit by hand). The frontend calls commands through the generated `commands` object (`import { commands } from '$lib/bindings'`), never stringly `invoke()`. Rust pushes events consumed with plain `listen()`: `ble_metrics`, `session_metrics`, `ble_error`, `ble_reconnect`, `ble_disconnected`; payload types are exported in `bindings.ts` too. CI regenerates the bindings and fails on drift. `docs/prd.md` is outdated; `bindings.ts` is the contract.

Do not duplicate Rust logic in TypeScript: flatten/labeling lives in Rust (`flatten_workout`), exposed via `flatten_workout_cmd`. Known accepted mirrors: `classify`/`zoneOf` in `lib/metrics.ts` (documented next to the Rust originals).

## Code style rules

- **Comments: 2 lines max.** Write the *why* (a constraint, a link to the spec, a mirror warning), never the *what*. If a comment is needed to explain *what* the code does, restructure the code instead. Mirror comments must name the mirrored source (file + symbol).
- **Cyclomatic complexity: keep functions small.** Aim for ≤ ~10 branches per function; prefer `match` and lookup tables over `if/else` chains, and extract a helper when nesting grows or a function stops fitting on one screen. Pure parsers stay branch-per-protocol-field — that is their shape; factor shared patterns into the FEATURES table rather than adding ad-hoc branches.
- **Lint discipline: zero warnings.** Clippy runs with `-D warnings` in CI, `svelte-check` must report 0 errors *and* 0 warnings. Do not silence a lint with `#[allow]`/`// eslint-disable`-style pragmas without a one-line justification comment.
- **Tests pin behavior, not implementation.** No tautological tests (asserting a constant equals itself, re-stating the code). Pin boundaries, error cases, and wire formats; a refactor should not require rewriting passing tests.
- **TDD: red → green.** For any new behavior or bug fix, write the failing test first and watch it fail, then implement until it passes. Applies to pure parsers, metrics, and frontend helpers (the code this repo exists to test); excluded for actors and hardware-facing code (see Key constraints).

## Key constraints

- **BLE device filtering by name prefix** — UUID-based filtering is unreliable on Windows WinRT. Filter by `"D500"` (trainer) and `"Polar"` (HRM).
- **Tests live in the same file** as the code they test (`#[cfg(test)]` module). No separate test files.
- **Actors are not unit-tested** — BleActor (BLE hardware), DbActor (SQLite I/O), and SessionActor (Tokio runtime) are validated manually or via integration tests only.
- **All project documentation** (issues, PRD, specs) is written in English. Conversation with the author is in French.
