#!/usr/bin/env bash
# Quality gate: blocks agent-generated megafiles.
# Fails if any source file exceeds MAX_FILE_LINES lines.
# Cyclomatic complexity is gated natively by clippy (clippy.toml, Cargo [lints]).
set -euo pipefail

MAX_FILE_LINES=1000
# bindings.ts is generated from Rust (never edit by hand) — not source.
EXCLUDED='src/lib/bindings.ts'

violations=0
while IFS= read -r file; do
  [ "$file" = "$EXCLUDED" ] && continue
  lines=$(wc -l < "$file")
  if [ "$lines" -gt "$MAX_FILE_LINES" ]; then
    echo "QUALITY GATE: $file: $lines lines (max $MAX_FILE_LINES)"
    violations=1
  fi
done < <(find src-tauri/src src \( -name '*.rs' -o -name '*.ts' -o -name '*.js' -o -name '*.svelte' \) -type f)

if [ "$violations" -ne 0 ]; then
  exit 1
fi
echo "File size gate OK (max $MAX_FILE_LINES lines)"
