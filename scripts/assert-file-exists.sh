#!/usr/bin/env bash
# Ensures the Netsuke build produced the expected artefact.
# Fails fast if the given file is missing.
set -euo pipefail

file="$1"

if [[ ! -f "$file" ]]; then
  echo "Expected build artefact '$file' to exist." >&2
  exit 1
fi
