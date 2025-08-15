#!/usr/bin/env bash
# Ensures the Netsuke build did not produce an unexpected artifact.
# If the artifact is present and `NINJA_MANIFEST` is set, the referenced
# Ninja manifest is dumped to stderr for debugging.
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $(basename "$0") <file>" >&2
  exit 64   # EX_USAGE
fi

file="$1"

if [[ -f "$file" ]]; then
  echo "Unexpected build artifact '$file' present." >&2
  if [[ -n "${NINJA_MANIFEST:-}" && -f "$NINJA_MANIFEST" ]]; then
    echo "Ninja manifest '$NINJA_MANIFEST' for debugging:" >&2
    echo "-----BEGIN NINJA MANIFEST-----" >&2
    cat "$NINJA_MANIFEST" >&2
    echo "-----END NINJA MANIFEST-----" >&2
  fi
  exit 1
fi
