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
BENCH_LOCK_DIR=${BENCH_LOCK_DIR:-$BENCH_ROOT.lock}

# Populated as "<label>|<clean seconds>|<incremental seconds>" rows.
results=()

# The benchmark touches BENCH_TOUCH_FILE to make the second pass incremental,
# and that file defaults to a tracked source. Leaving it newer than the ordinary
# `target/` outputs would silently force the developer's next real build to redo
# work, long after the benchmark finished, so the timestamp is restored on exit —
# including when a measurement fails or the run is interrupted.
#
# This holds a scratch file whose own timestamp is the one to put back, rather
# than an epoch number: `touch -r` is POSIX, whereas reading the stamp with
# `stat -c` and replaying it with `touch -d @epoch` is GNU-only and fails on
# macOS. The benchmark is reachable there, because the capability check tolerates
# a non-Linux host rather than aborting.
BENCH_TOUCH_STAMP=

restore_touch_file() {
  [ -n "$BENCH_TOUCH_STAMP" ] || return 0
  # Swallowing this would be the worst of both worlds: the developer keeps the
  # consequence — a source file left newer than the build outputs, so the next
  # real build silently redoes work — and loses the only notice that it
  # happened. The trap must not abort the run, so warn rather than fail, and say
  # enough that the state can be checked and put right by hand.
  touch -r "$BENCH_TOUCH_STAMP" "$BENCH_TOUCH_FILE" ||
    note "failed to restore the timestamp of $BENCH_TOUCH_FILE; it is left newer than before the benchmark, so the next build will redo work. Check it with: ls -l $BENCH_TOUCH_FILE"
  rm -f "$BENCH_TOUCH_STAMP"
  BENCH_TOUCH_STAMP=
}

# Two benchmark runs in one checkout are not independent: they share the variant
# target directories, so one run's `rm -rf` for its clean pass deletes the other
# run's warm cache mid-measurement, and they share the touch file, so the second
# run captures a stamp the first has already moved and restores that instead of
# the original. The result is neither a crash nor a comparable figure — it is a
# plausible-looking table and a permanently newer source file.
#
# So take the run exclusively rather than documenting the hazard. `mkdir` is the
# portable atomic test-and-set: it succeeds for exactly one caller and needs no
# `flock`, which is util-linux and absent on macOS, where this script is
# reachable because the capability check tolerates a non-Linux host.
BENCH_LOCK_HELD=

acquire_bench_lock() {
  mkdir -p -- "$(dirname -- "$BENCH_LOCK_DIR")"
  mkdir -- "$BENCH_LOCK_DIR" 2>/dev/null || fail \
    "another benchmark run holds $BENCH_LOCK_DIR; wait for it to finish, or remove that directory if it was left behind by a killed run"
  BENCH_LOCK_HELD=1
}

release_bench_lock() {
  [ -n "$BENCH_LOCK_HELD" ] || return 0
  rmdir -- "$BENCH_LOCK_DIR" 2>/dev/null || true
  BENCH_LOCK_HELD=
}

# One handler for both, so an interrupted run releases the lock as well as
# restoring the timestamp. Each half is idempotent, so EXIT firing after INT or
# TERM is harmless.
cleanup() {
  restore_touch_file
  release_bench_lock
}

trap cleanup EXIT INT TERM

# Wall-clock seconds for a command, to one decimal place. EPOCHREALTIME keeps
# the measurement sub-second without shelling out to an external timer.
time_command() {
  local start=${EPOCHREALTIME/,/.} end
  # Suppress stdout only. This function's stdout is captured by the caller, so
  # build chatter would corrupt the measurement, but stderr must reach the
  # terminal: without it a failing build reports only "benchmark command
  # failed" and hides the compiler or linker diagnostic that explains why.
  "$@" >/dev/null || fail "benchmark command failed: $*"
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
  if [ -z "$BENCH_TOUCH_STAMP" ]; then
    BENCH_TOUCH_STAMP=$(mktemp)
    touch -r "$BENCH_TOUCH_FILE" "$BENCH_TOUCH_STAMP"
  fi
  touch "$BENCH_TOUCH_FILE"
  incremental=$(time_command "$@")

  unset CARGO_TARGET_DIR
  results+=("$label|$clean|$incremental")
}

# Render the accumulated rows as a Markdown table, ready to paste into the
# developers' guide.
report() {
  local row label clean incremental
  printf '\n| Variant | Clean build (s) | Incremental build (s) |\n'
  printf '| --- | --- | --- |\n'
  for row in "${results[@]}"; do
    IFS='|' read -r label clean incremental <<<"$row"
    printf '| %s | %s | %s |\n' "$label" "$clean" "$incremental"
  done
}

# Measure the default path first so its numbers are not attributed to a warm
# page cache created by the accelerated run.
main() {
  local toolchain
  toolchain=$(cranelift_toolchain)

  # Before the first `rm -rf` or `touch`, so a rejected run leaves the holder's
  # state untouched.
  acquire_bench_lock

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
