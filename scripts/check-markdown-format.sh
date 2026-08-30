#!/usr/bin/env bash
# Verifies that Markdown sources are already in the canonical form `make fmt`
# produces, without modifying any tracked file.
#
# `mdtablefix` owns table padding and paragraph wrapping. It has no check-only
# mode, so this script compares its output for each file against the file on
# disk. Keep the flags in step with the `mdtablefix` invocation in
# `mdformat-all`, which `make fmt` runs.
#
# `make fmt` also applies `markdownlint-cli2 --fix` after `mdtablefix`, but that
# pass is deliberately not replayed here. `make markdownlint` already rejects
# any lint violation, so on a passing tree `--fix` has nothing to change.
# Comparing against `mdtablefix` alone additionally surfaces documents the two
# tools would fight over -- a heading nested inside an ordered list, for
# instance, ends the list for `mdtablefix` while `MD029` keeps renumbering it --
# which indicates malformed Markdown that should be restructured.
set -euo pipefail

if [[ $# -eq 0 ]]; then
  echo "Usage: $(basename "$0") <file>..." >&2
  exit 64 # EX_USAGE
fi

MDTABLEFIX="${MDTABLEFIX:-mdtablefix}"

if ! command -v "$MDTABLEFIX" >/dev/null 2>&1; then
  echo "$(basename "$0"): '$MDTABLEFIX' is not installed or not on PATH." >&2
  exit 127
fi

formatted="$(mktemp)"
trap 'rm -f "$formatted"' EXIT

unformatted=()
for file in "$@"; do
  "$MDTABLEFIX" --wrap --renumber --breaks --ellipsis --fences "$file" \
    >"$formatted"
  if ! cmp -s "$formatted" "$file"; then
    unformatted+=("$file")
  fi
done

if [[ ${#unformatted[@]} -gt 0 ]]; then
  echo "The following Markdown files are not formatted; run 'make fmt':" >&2
  printf '  %s\n' "${unformatted[@]}" >&2
  exit 1
fi
