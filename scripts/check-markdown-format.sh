#!/usr/bin/env bash
# Verifies that Markdown sources are already in the canonical form `make fmt`
# produces, without modifying any tracked file.
#
# `mdtablefix` owns table padding and paragraph wrapping. It has no check-only
# mode, so this script formats staged copies in one batch, then compares each
# result against the corresponding source without modifying tracked files.
# `mdtablefix` emits LF, whereas Git can check text out with CRLF on Windows;
# the comparison accepts only either exact line-ending form. Keep the flags in
# step with the `mdtablefix` invocation in
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

staged_directory="$(mktemp -d)"
trap 'rm -rf "$staged_directory"' EXIT

original_files=()
staged_files=()
for file in "$@"; do
  staged_file="$staged_directory/${#original_files[@]}.md"
  cp -- "$file" "$staged_file"
  original_files+=("$file")
  staged_files+=("$staged_file")
done

"$MDTABLEFIX" --in-place --wrap --renumber --breaks --ellipsis --fences \
  "${staged_files[@]}"

unformatted=()
for index in "${!original_files[@]}"; do
  original_file="${original_files[$index]}"
  staged_file="${staged_files[$index]}"
  if ! cmp -s "$staged_file" "$original_file" \
    && ! sed $'s/$/\r/' "$staged_file" | cmp -s - "$original_file"; then
    unformatted+=("$original_file")
  fi
done

if [[ ${#unformatted[@]} -gt 0 ]]; then
  echo "The following Markdown files are not formatted; run 'make fmt':" >&2
  printf '  %s\n' "${unformatted[@]}" >&2
  exit 1
fi
