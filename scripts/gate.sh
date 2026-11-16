#!/usr/bin/env bash
# Usage: bash scripts/gate.sh [--quick | KEY...]   (no args: every gate in .gates.yml)
set -euo pipefail
cd "$(dirname "$0")/.."

GATES_FILE=.gates.yml
QUICK=(format lint typecheck test)

gate_keys() { sed -n 's/^\([a-z_]*\):.*/\1/p' "$GATES_FILE" | grep -vx stack; }
gate_cmd() { sed -n "/^$1:/{s/^$1:[[:space:]]*//p;q}" "$GATES_FILE" | tr -d '\r'; }

# WSL interop: cargo may only exist as cargo.exe; expose it as `cargo` to gate commands.
if ! command -v cargo >/dev/null 2>&1 && command -v cargo.exe >/dev/null 2>&1; then
  shim=$(mktemp -d)
  ln -s "$(command -v cargo.exe)" "$shim/cargo"
  PATH="$shim:$PATH"
fi

if [ $# -eq 0 ]; then
  mapfile -t keys < <(gate_keys)
elif [ "$1" = --quick ]; then
  keys=("${QUICK[@]}")
else
  keys=("$@")
fi

for key in "${keys[@]}"; do
  if [ -z "$(gate_cmd "$key")" ]; then
    echo "Unknown gate '$key'. Available: $(gate_keys | tr '\n' ' ')" >&2
    exit 2
  fi
done

if [[ " ${keys[*]} " == *" sast "* ]] && ! cargo deny --version >/dev/null 2>&1; then
  echo "cargo-deny not found, installing (one-time, takes a few minutes)..."
  cargo install cargo-deny --locked
fi

failed=()
for key in "${keys[@]}"; do
  cmd=$(gate_cmd "$key")
  echo "== $key: $cmd =="
  # Subshell: a `cd` inside one gate must not leak into the next.
  if ! (set -euo pipefail; eval "$cmd"); then
    failed+=("$key")
  fi
done

if [ ${#failed[@]} -ne 0 ]; then
  echo "GATE FAILED: ${failed[*]}"
  exit 1
fi
echo "GATE OK: ${keys[*]}"
