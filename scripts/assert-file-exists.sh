#!/usr/bin/env bash
# Ensures the Netsuke build produced the expected artefact.
# Fails fast if the given file is missing.
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $(basename "$0") <file>" >&2
  exit 64   # EX_USAGE
fi

file="$1"

if [[ ! -f "$file" ]]; then
  echo "Expected build artefact '$file' to exist." >&2
  exit 1
fi
