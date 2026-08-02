#!/usr/bin/env bash
# Shared helpers for the opt-in mold + Cranelift local build acceleration.
#
# Sourced by scripts/install-dev-fast.sh and scripts/dev-fast-check.sh so both
# resolve the pinned versions and emit diagnostics identically. Every message is
# prefixed `dev-fast:` and written to stderr, matching the `prover-tools:`
# convention used by the Kani and Verus targets.

set -euo pipefail

# Locate the repository from this file rather than from the working directory,
# so the entry points run correctly when invoked directly and not only through
# the `make dev-*` recipes that used to supply every pin path. `BASH_SOURCE[0]`
# is this file even when sourced, which is what makes the derivation reliable.
DEV_FAST_HELPER_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
DEV_FAST_REPO_ROOT=$(cd -- "$DEV_FAST_HELPER_DIR/.." && pwd)

# Pin files default to their committed locations. An explicit override still
# wins, which is what lets the tests point the scripts at fixtures. `read_pin`
# validates whichever path is selected, so a missing or empty file is reported
# the same way whether it came from a default or an override.
MOLD_VERSION_FILE="${MOLD_VERSION_FILE:-$DEV_FAST_REPO_ROOT/tools/mold/VERSION}"
MOLD_SHA256SUMS_FILE="${MOLD_SHA256SUMS_FILE:-$DEV_FAST_REPO_ROOT/tools/mold/SHA256SUMS}"
RUST_TOOLCHAIN_FILE="${RUST_TOOLCHAIN_FILE:-$DEV_FAST_REPO_ROOT/rust-toolchain.toml}"

# Prefix for the mold installation tree. The `dev-*` recipes prepend this
# prefix's `bin/` to PATH -- this exact prefix, not a hard-coded ~/.local -- so
# an overridden DEV_FAST_PREFIX is the one that wins PATH resolution for both
# `dev-fast-check` and `-fuse-ld=mold`. Invoking these scripts outside `make`
# means arranging that PATH order separately.
DEV_FAST_PREFIX="${DEV_FAST_PREFIX:-$HOME/.local}"

# shellcheck disable=SC2034 # consumed by the scripts that source this file.
CRANELIFT_COMPONENT='rustc-codegen-cranelift-preview'

# Emit a diagnostic. Always stderr, so a caller may capture a helper's stdout
# without the diagnostics contaminating the captured value.
note() { printf 'dev-fast: %s\n' "$*" >&2; }

# Emit a diagnostic and abort. Used for conditions no caller can recover from,
# such as a missing pin file or an unverifiable download.
fail() {
  printf 'dev-fast: %s\n' "$*" >&2
  exit 1
}

# Read a single-line version pin, trimming only its leading and trailing
# whitespace.
#
# A missing, blank, or multi-line pin aborts rather than yielding a version that
# would silently produce a nonsensical download URL. Deleting every whitespace
# character instead would corrupt rather than reject: a stray space would turn
# `1.2 3` into `1.23`, and a second line would concatenate into `1.2.34.5.6`.
# Internal whitespace is likewise rejected, because no pin legitimately contains
# any and silently rewriting one is worse than refusing it.
read_pin() {
  local file=$1 value lines
  [ -f "$file" ] || fail "missing version pin: $file"
  lines=$(grep -c '' <"$file")
  [ "$lines" -le 1 ] || fail "expected one line in version pin: $file, found $lines"
  # `$(...)` strips trailing newlines; the parameter expansions trim spaces and
  # tabs from each end without touching anything between them.
  value=$(cat -- "$file")
  value=${value#"${value%%[![:space:]]*}"}
  value=${value%"${value##*[![:space:]]}"}
  [ -n "$value" ] || fail "empty version pin: $file"
  case $value in
    *[[:space:]]*) fail "version pin contains whitespace: $file" ;;
  esac
  printf '%s' "$value"
}

# The pinned mold release tag, e.g. "2.41.0".
mold_version() { read_pin "$MOLD_VERSION_FILE"; }

# The repository's toolchain, read from `rust-toolchain.toml`.
#
# Deliberately the same toolchain the ordinary gates use, not a second pin.
# The tree borrow-checks only under Polonius on that dated nightly (ADR-006),
# so a separate dev-fast nightly would let the fast loop and the gate disagree
# about which borrows are legal.
cranelift_toolchain() {
  local file=$RUST_TOOLCHAIN_FILE value
  [ -f "$file" ] || fail "missing version pin: $file"
  value=$(awk -F'"' '/^[[:space:]]*channel[[:space:]]*=/ { print $2; exit }' "$file")
  [ -n "$value" ] || fail "no channel found in: $file"
  printf '%s' "$value"
}

# Whether the host can use mold at all; it ships for Linux only.
is_linux() { [ "$(uname -s)" = 'Linux' ]; }

# mold publishes per-architecture tarballs; map `uname -m` onto those names.
mold_arch() {
  local machine
  machine=$(uname -m)
  case "$machine" in
    x86_64 | amd64) printf 'x86_64' ;;
    aarch64 | arm64) printf 'aarch64' ;;
    *) fail "unsupported architecture for the pinned mold release: $machine" ;;
  esac
}

# The version of whichever mold PATH resolves to, or a non-zero status when
# that mold cannot be run.
installed_mold_version() {
  # `mold --version` prints e.g. "mold 2.41.0 (compatible with GNU ld)". Capture
  # first so a failing mold propagates its status instead of being masked by the
  # exit status of a downstream awk.
  local output
  output=$(mold --version 2>/dev/null) || return 1
  printf '%s' "$output" | awk 'NR == 1 { print $2 }'
}

# Whether the Cranelift backend is installed for the given toolchain. rustup
# reports the component with a host-triple suffix, so match on the prefix.
has_cranelift_component() {
  local toolchain=$1
  rustup component list --installed --toolchain "$toolchain" 2>/dev/null |
    grep -q '^rustc-codegen-cranelift'
}
