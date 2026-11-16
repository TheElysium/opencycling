# AGENTS.md

Guidance for AI agents in this repo. This is a map, not a manual — read the
code for anything not covered here.

## Commands

Gates are declared once in `.gates.yml` (`format`, `lint`, `typecheck`, `test`, `build` = bindings
drift, `sast` = `cargo deny` + `pnpm audit`). `scripts/gate.sh` runs them and CI calls the same script.

- `bash scripts/gate.sh --quick`: format + lint + typecheck + test, the inner loop.
- `bash scripts/gate.sh lint test`: only the named gates. Failures are summarized at the end.
- **Run `bash scripts/gate.sh` (all gates) before declaring a task done.**

Single test: `cargo test <name>` (from `src-tauri/`), `pnpm vitest run <file>`. Regen bindings:
`cargo run --bin export_bindings`. Run the app: `pnpm tauri dev` (`OPENYCLING_SIM=1` for the BLE sim).

Rust toolchain is pinned in `rust-toolchain.toml`; bump it deliberately, new clippy lints come with it.

## Lint rules (all errors, zero warnings)

- Rust: rustfmt defaults; clippy default groups with `-D warnings` on all targets;
  `cognitive_complexity` deny at 15 (`clippy.toml`); `unsafe_code` forbid (`Cargo.toml [lints]`).
- Frontend: ESLint js/typescript-eslint/svelte recommended, `complexity` 15, `max-lines-per-function`
  300, `--max-warnings 0`; `svelte-check --fail-on-warnings` with TS `strict`.
- Any source file over 1000 lines fails (`scripts/check_size.sh`, `bindings.ts` excluded).
- Dependencies: advisories, licenses and sources in `src-tauri/deny.toml`; npm prod deps fail on high.

## Architecture

Tauri v2 desktop app: SvelteKit 5 (runes) frontend + Rust backend.

- `src-tauri/src/{ble,workout,plan}/` — pure parsers/domain logic, no I/O. Keep them that way.
- `src-tauri/src/{ble,session,db}/` — Tokio actors, talk only via `mpsc`. Each has `command.rs`
  (`*Handle` + `*Command` enum, called from Tauri commands) and `actor.rs`/`types.rs` (the loop).
  Wired in `lib.rs::run()`.
- `src-tauri/src/metrics.rs` mirrors `src/lib/metrics.ts` (`classify`/`zoneOf`) — the one accepted
  frontend/backend duplication. Don't add others; put shared logic in Rust and expose a command.
- Errors flow through `AppError` (`src-tauri/src/errors.rs`), serialized as a plain string.
- `src/routes/` — SvelteKit pages. `src/lib/*.svelte.ts` — shared rune stores. `src/lib/components/` —
  reusable UI.
- Tauri bridge is typed end-to-end via tauri-specta: `src/lib/bindings.ts` is **generated**, never
  hand-edit it; CI fails on drift. Frontend always calls `commands` from bindings, never raw
  `invoke()`. `docs/prd.md` is outdated — `bindings.ts` is the contract.

## Code style

- Comments: 2 lines max, explain *why* not *what*.
- Keep cyclomatic complexity low (clippy/eslint gate at 15); a new `#[expect]`/eslint-disable needs
  a one-line justification.
- Zero warnings tolerated (`clippy -D warnings`, `svelte-check`).
- Tests pin behavior, not implementation — no tautological tests.
- TDD (red -> green) for new behavior/bug fixes in parsers, metrics, frontend helpers. Not required
  for actors or hardware-facing code.

## Key constraints

- BLE filtering is by device name prefix (`D500`, `Polar`), not UUID — UUID filtering is unreliable
  on Windows WinRT.
- Tests live in the same file (`#[cfg(test)]`), no separate test files.
- Actors (Ble/Db/Session) are not unit-tested — manual/integration only.
- Docs (issues, PRD, specs) in English; conversation with the author is in French.
