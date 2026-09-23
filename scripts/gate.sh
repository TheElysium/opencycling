#!/usr/bin/env bash
# Full verification gate for agents: run before declaring any task done.
# Commands mirror .gates.yml (what agents run) and ci.yml (what CI enforces):
# fmt, clippy (+ complexity gate), tests, bindings drift, audit, file-size
# gate, frontend lint, type check and unit tests. Change all three together.
set -euo pipefail

# Cargo may be installed outside the default PATH (e.g. WSL interop: cargo.exe).
CARGO=cargo
if ! command -v cargo >/dev/null 2>&1 && command -v cargo.exe >/dev/null 2>&1; then
  CARGO=cargo.exe
fi

cd src-tauri
echo "== cargo fmt =="
$CARGO fmt --all --check
echo "== cargo clippy (incl. cognitive-complexity gate) =="
$CARGO clippy --all-targets -- -D warnings
echo "== cargo test =="
$CARGO test

# The tauri-specta bridge is generated: fail if the committed copy drifted.
echo "== bindings drift =="
$CARGO run --bin export_bindings
git diff --exit-code -- ../src/lib/bindings.ts

echo "== cargo audit =="
if ! command -v cargo-audit >/dev/null 2>&1 && ! command -v cargo-audit.exe >/dev/null 2>&1; then
  echo "cargo-audit not found, installing (one-time, takes a few minutes)..."
  $CARGO install cargo-audit --locked
fi
$CARGO audit

echo "== file size gate =="
cd ..
bash scripts/check_size.sh

echo "== pnpm lint =="
pnpm lint
echo "== pnpm check =="
pnpm check
echo "== pnpm test =="
pnpm test

echo "GATE OK"
