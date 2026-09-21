#!/usr/bin/env bash
# Benchmark the three debug build shapes: the platform-linker baseline, the
# repository's `mold` default, and that default with the parallel `rustc`
# frontend added. Separating the linker from the frontend is the point: they
# pay off at different points in the build, so one row for both would hide
# which of them is earning its keep.
#
# Each variant is measured twice: a clean build from an empty target directory,
# and an incremental rebuild after touching the binary's entry point. Variants
# use separate target directories so neither warms nor invalidates the other's
# cache, and neither disturbs the working `target/` tree. Results are printed as
# a Markdown table so the developers' guide can be regenerated verbatim.
#
# The variants are selected by environment override rather than by a Cargo
# configuration fragment. `.cargo/config.toml` is the committed default, so the
# baseline is expressed by displacing its `rustflags` entirely, and the two
# accelerated rows differ only by one flag.

set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=scripts/build-tools-common.sh
. "$script_dir/build-tools-common.sh"

: "${CARGO:=cargo}"
# Supplied by the Makefile from the same variables the gate targets compose, so
# a flag cannot be benchmarked in a shape the gates do not actually use.
: "${STANDARD_THREADS_FLAG:=-Zthreads=8}"
: "${STANDARD_MOLD_FLAG:=-Clink-arg=-fuse-ld=mold}"

# The timer below reads EPOCHREALTIME, which Bash gained in 5.0. Fail here with
# a named prerequisite rather than silently reporting every duration as zero.
[ "${BASH_VERSINFO[0]:-0}" -ge 5 ] ||
  fail "bash 5.0 or newer is required to benchmark; found ${BASH_VERSION:-unknown}"

BENCH_ROOT=${BENCH_ROOT:-target/bench}
BENCH_BIN=${BENCH_BIN:-netsuke}
BENCH_TOUCH_FILE=${BENCH_TOUCH_FILE:-src/main.rs}
BENCH_LOCK_DIR=${BENCH_LOCK_DIR:-$BENCH_ROOT.lock}
# How many times each variant is measured. One sample cannot separate a variant
# from the host, and shared state that no target directory isolates — page-cache
# warmth, other load — is exactly what the ordering bias lives in.
BENCH_REPEATS=${BENCH_REPEATS:-2}
# Refused rather than clamped. A run with nought repeats prints an empty table
# and exits nought, and a run with one prints a table that looks exactly like a
# valid one while carrying the single-sample bias this script exists to remove.
# Both are worse than a named failure, because neither is visible in the output
# a reader pastes into the guide.
#
# The empty string is not among the refusals: `:-` above has already replaced it
# with the default, as it does for every other `BENCH_` variable, so a pattern
# for it here could never match and would read as a guard that holds.
case $BENCH_REPEATS in
  *[!0-9]*) fail "BENCH_REPEATS must be a whole number of at least 2; got '$BENCH_REPEATS'" ;;
esac
[ "$BENCH_REPEATS" -ge 2 ] ||
  fail "BENCH_REPEATS must be a whole number of at least 2; got '$BENCH_REPEATS'"

# The seed the variant order is drawn from.
#
# The shuffle is part of the measurement, so the draw is an input rather than
# ambient state: a table that cannot be reproduced cannot be checked, and a
# script reading Bash's global `$RANDOM` directly offers no way to pin the
# order it produced. An unset seed draws one and prints it, so a run is
# reproducible after the fact by passing that value back.
BENCH_SEED=${BENCH_SEED:-$RANDOM}
case $BENCH_SEED in
  *[!0-9]*) fail "BENCH_SEED must be a whole number; got '$BENCH_SEED'" ;;
esac

# Populated as "<label>|<clean seconds>|<incremental seconds>" rows, in the
# order the samples were measured.
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

# The environment every measured build runs under.
#
# Each variant already assigns `RUSTFLAGS`; these three are the difference
# between compiling, retrieving, and measuring a different build entirely.
#
# A developer shell on a shared host commonly exports a `RUSTC_WRAPPER` that
# chains to `sccache`. With one in force a variant's first clean pass populates
# the cache and every later pass reads it back, so the table reports cache
# retrieval times under variant labels and the ordering of the rows decides the
# result. Worse, the flags are part of the cache key, so the variants warm each
# other unevenly and the bias is invisible. Both wrapper variables are assigned
# empty rather than unset: Cargo honours `RUSTC_WORKSPACE_WRAPPER`
# independently, so clearing one alone still leaves the workspace's own crates
# wrapped.
#
# `CARGO_ENCODED_RUSTFLAGS` is removed rather than assigned. Cargo checks it
# before `RUSTFLAGS` and uses the first source it finds, so an inherited value
# would let every variant compile with the same flags while the table still
# showed three different rows — the comparison would be void, and nothing in
# the output would say so. Assigning it empty would not do: an empty encoded
# list is still a source, and would displace every variant's own `RUSTFLAGS`.
#
# Measured 2026-09-17 on a 32-core host: with the wrapper inherited, the same
# variant's clean build ranged from 37 s to 154 s across three runs, and the
# ordering of the rows reversed the verdict twice.
bench_env=(-u CARGO_ENCODED_RUSTFLAGS RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER='')

# The variants, as "<slug>|<label>|<RUSTFLAGS>" rows.
#
# A single table rather than a call site per variant, because the only thing
# that distinguishes one row from another is its `RUSTFLAGS`: the build command
# is identical for all of them, so a variant whose flags drifted is the whole of
# the bug this can have. Keeping the flags beside the slug also lets a test
# compare the two without re-deriving either.
#
# Assigning `RUSTFLAGS` at all, even to nothing, displaces every `rustflags`
# table in `.cargo/config.toml`, which is what makes the baseline row the
# pre-standard build exactly. `$STANDARD_MOLD_FLAG` and the threaded row's
# suffix are filled in by `main`, which is also where a non-Linux host drops the
# mold row: an empty linker flag there would make it a second measurement of the
# baseline under a caption claiming otherwise.
#
# The labels are backticked because the developers' guide embeds this table
# verbatim, and the repository spelling gate reads a bare "mold" as "mould".
# shellcheck disable=SC2016 # the backticks are Markdown, not a subshell.
variants=(
  'default|Default (platform linker)|'
  'mold|`mold`|@MOLD@'
  'mold-threads|`mold`, parallel frontend|@THREADS@@MOLD_SUFFIX@'
)

# Print one field of the variant named by `slug`.
#
# Deliberately a lookup rather than parallel arrays indexed by position: the
# shuffle reorders the slugs, so anything positional would have to be permuted
# in step and would silently mis-attribute a label the moment the two lists
# disagreed.
variant_field() {
  local want=$1 field=$2 entry
  for entry in "${variants[@]}"; do
    IFS='|' read -r slug label flags <<<"$entry"
    [ "$slug" = "$want" ] || continue
    case $field in
      label) printf '%s' "$label" ;;
      flags) printf '%s' "$flags" ;;
      *) fail "unknown variant field: $field" ;;
    esac
    return 0
  done
  fail "no variant named: $want"
}

# Usage: measure_variant <slug> <toolchain>
# The slug names the variant's private target directory and selects its flags.
measure_variant() {
  local slug=$1 toolchain=$2
  local label flags clean incremental
  label=$(variant_field "$slug" label)
  flags=$(variant_field "$slug" flags)
  export CARGO_TARGET_DIR="$BENCH_ROOT/$slug"
  # Cargo can be told to keep intermediates outside the target directory. If a
  # caller has done that, every variant shares one build directory, the `rm -rf`
  # below stops making the next pass clean, and the table reports three warm
  # builds while looking exactly like three cold ones. Drop the override so
  # intermediates land under each variant's own directory.
  unset CARGO_BUILD_BUILD_DIR

  note "measuring $label (clean)"
  rm -rf "$CARGO_TARGET_DIR"
  clean=$(time_command env "${bench_env[@]}" \
    RUSTUP_TOOLCHAIN="$toolchain" RUSTFLAGS="$flags" \
    "$CARGO" build --bin "$BENCH_BIN")

  note "measuring $label (incremental)"
  if [ -z "$BENCH_TOUCH_STAMP" ]; then
    BENCH_TOUCH_STAMP=$(mktemp)
    touch -r "$BENCH_TOUCH_FILE" "$BENCH_TOUCH_STAMP"
  fi
  touch "$BENCH_TOUCH_FILE"
  incremental=$(time_command env "${bench_env[@]}" \
    RUSTUP_TOOLCHAIN="$toolchain" RUSTFLAGS="$flags" \
    "$CARGO" build --bin "$BENCH_BIN")

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

# A permutation of the variant slugs, as space-separated words.
#
# The order is shuffled rather than fixed, and repeated, because separate target
# directories isolate build artefacts and nothing else. Page-cache warmth and
# other tenants on a shared host are not isolated by any directory, and they are
# where the ordering bias lives: the recorded 2026-09-17 attempt reversed its
# verdict twice when the rows were reversed. Drawing a fresh order per sample
# spreads that bias across the variants instead of pinning it to one, and
# repeating makes it visible as spread rather than hidden in a single number.
#
# A Fisher-Yates draw, which is fine here and would not be for anything
# security-relevant. `shuf` is not portable to macOS, where this benchmark is
# reachable because the capability check tolerates a non-Linux host; bash is
# already required for `EPOCHREALTIME`.
#
# The draw comes from `bench_random` rather than from `$RANDOM` directly. That
# one function is the whole of this script's dependency on randomness, so it is
# the only thing a caller has to pin to reproduce a table: seeding it from
# `BENCH_SEED` makes an order replayable, and a test can compare two runs of the
# same seed without reaching inside the shuffle.
#
# The permutation lands in `BENCH_ORDER` rather than on standard output,
# because a caller reading it back through `$(...)` would fork, and Bash
# reseeds `$RANDOM` in a subshell. Every sample would then draw from a fresh
# sequence: the orders would still look perfectly shuffled, the run would still
# print a seed, and that seed would decide nothing. This was the actual
# behaviour until the fork was removed.
permuted_slugs() {
  BENCH_ORDER=("$@")
  local index swap pick
  for ((index = ${#BENCH_ORDER[@]} - 1; index > 0; index--)); do
    bench_random
    pick=$((BENCH_RANDOM_VALUE % (index + 1)))
    swap=${BENCH_ORDER[index]}
    BENCH_ORDER[index]=${BENCH_ORDER[pick]}
    BENCH_ORDER[pick]=$swap
  done
}

# The single source of randomness, seeded by `seed_bench_random` below.
#
# Bash's `$RANDOM` is a sequence rather than a fresh draw each time, and
# assigning to it sets the sequence's starting point, which is what makes a
# seeded run reproducible.
#
# The draw lands in a variable rather than on standard output because reading
# it back through a command substitution would fork, and Bash reseeds `$RANDOM`
# in a subshell. Every draw would then come from a fresh sequence, the seed
# would decide nothing, and the reproducibility this boundary exists for would
# be quietly absent.
bench_random() { BENCH_RANDOM_VALUE=$RANDOM; }

# Pin the sequence `bench_random` walks.
seed_bench_random() { RANDOM=$1; }

# Measure every variant `BENCH_REPEATS` times, in a freshly shuffled order each
# time, and print the table followed by the record of what was actually run.
#
# The executed order is printed rather than implied, because the shuffle is not
# reconstructible after the fact. Without it, a table whose rows disagree with
# an earlier run cannot be told apart from one whose variant order differed —
# which is the exact confusion this rework exists to remove.
main() {
  local slug sample
  local -a measured=() slugs=()
  local toolchain
  toolchain=$(pinned_toolchain)

  # Printed before anything is measured, so an aborted run still says which
  # seed produced the order it had reached.
  seed_bench_random "$BENCH_SEED"
  printf 'order seed: %s\n' "$BENCH_SEED"

  # A non-Linux host loses the linker row entirely and keeps the other two, so
  # the threaded row's caption names the frontend as what it varies.
  if is_linux; then
    variants[1]=${variants[1]//@MOLD@/$STANDARD_MOLD_FLAG}
    variants[2]=${variants[2]//@THREADS@/$STANDARD_THREADS_FLAG}
    variants[2]=${variants[2]//@MOLD_SUFFIX@/ $STANDARD_MOLD_FLAG}
  else
    variants[2]=${variants[2]//@THREADS@/$STANDARD_THREADS_FLAG}
    variants[2]=${variants[2]//@MOLD_SUFFIX@/}
    variants[2]=${variants[2]//\`mold\`, parallel frontend/Platform linker, parallel frontend}
    unset 'variants[1]'
    # Re-index, so the shuffle ranges over what is left rather than over a hole.
    variants=("${variants[@]}")
    note "mold is Linux-only; measuring on $(uname -s) without a linker change, so the mold row is omitted and the threaded row varies the frontend alone"
  fi

  local entry
  for entry in "${variants[@]}"; do
    slugs+=("${entry%%|*}")
  done

  # Before the first `rm -rf` or `touch`, so a rejected run leaves the holder's
  # state untouched.
  acquire_bench_lock

  for ((sample = 0; sample < BENCH_REPEATS; sample++)); do
    permuted_slugs "${slugs[@]}"
    printf 'order sample %s: %s\n' "$((sample + 1))" "${BENCH_ORDER[*]}"
    for slug in "${BENCH_ORDER[@]}"; do
      measure_variant "$slug" "$toolchain"
      measured+=("$slug")
    done
  done
  printf 'order measured: %s\n' "${measured[*]}"

  report
}

main "$@"
