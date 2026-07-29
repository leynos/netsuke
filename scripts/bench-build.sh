#!/usr/bin/env bash
# Benchmark the default (LLVM + platform linker) debug build against the opt-in
# mold + Cranelift path.
#
# Each variant is measured twice: a clean build from an empty target directory,
# and an incremental rebuild after touching the binary's entry point. Variants
# use separate target directories so neither warms nor invalidates the other's
# cache, and neither disturbs the working `target/` tree. Results are printed as
# a Markdown table so the developers' guide can be regenerated verbatim.

set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=scripts/dev-fast-common.sh
. "$script_dir/dev-fast-common.sh"

: "${CARGO:=cargo}"
: "${DEV_FAST_CONFIG:?DEV_FAST_CONFIG must be set}"

# The timer below reads EPOCHREALTIME, which Bash gained in 5.0. Fail here with
# a named prerequisite rather than silently reporting every duration as zero.
[ "${BASH_VERSINFO[0]:-0}" -ge 5 ] ||
  fail "bash 5.0 or newer is required to benchmark; found ${BASH_VERSION:-unknown}"

BENCH_ROOT=${BENCH_ROOT:-target/bench}
BENCH_BIN=${BENCH_BIN:-netsuke}
BENCH_TOUCH_FILE=${BENCH_TOUCH_FILE:-src/main.rs}

# Populated as "<label>|<clean seconds>|<incremental seconds>" rows.
results=()

# Wall-clock seconds for a command, to one decimal place. EPOCHREALTIME keeps
# the measurement sub-second without shelling out to an external timer.
time_command() {
  local start=${EPOCHREALTIME/,/.} end
  "$@" >/dev/null 2>&1 || fail "benchmark command failed: $*"
  end=${EPOCHREALTIME/,/.}
  LC_ALL=C awk -v start="$start" -v end="$end" 'BEGIN { printf "%.1f", end - start }'
}

# Usage: measure_variant <slug> <label> <command...>
# The slug names the variant's private target directory; the label is the table
# caption for that row.
measure_variant() {
  local slug=$1 label=$2
  local clean incremental
  shift 2
  export CARGO_TARGET_DIR="$BENCH_ROOT/$slug"

  note "measuring $label (clean)"
  rm -rf "$CARGO_TARGET_DIR"
  clean=$(time_command "$@")

  note "measuring $label (incremental)"
  touch "$BENCH_TOUCH_FILE"
  incremental=$(time_command "$@")

  unset CARGO_TARGET_DIR
  results+=("$label|$clean|$incremental")
}

report() {
  local row label clean incremental
  printf '\n| Variant | Clean build (s) | Incremental build (s) |\n'
  printf '| --- | --- | --- |\n'
  for row in "${results[@]}"; do
    IFS='|' read -r label clean incremental <<<"$row"
    printf '| %s | %s | %s |\n' "$label" "$clean" "$incremental"
  done
}

main() {
  local toolchain
  toolchain=$(cranelift_toolchain)

  measure_variant default 'Default (LLVM, platform linker)' \
    "$CARGO" build --bin "$BENCH_BIN"

  # The label is backticked because the developers' guide embeds this table
  # verbatim, and the repository spelling gate reads a bare "mold" as "mould".
  # shellcheck disable=SC2016 # the backticks are Markdown, not a subshell.
  measure_variant dev-fast 'dev-fast (Cranelift, `mold`)' \
    env RUSTUP_TOOLCHAIN="$toolchain" \
    "$CARGO" --config "$DEV_FAST_CONFIG" build --bin "$BENCH_BIN"

  report
}

main "$@"
