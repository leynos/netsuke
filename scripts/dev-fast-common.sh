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
CRANELIFT_TOOLCHAIN_FILE="${CRANELIFT_TOOLCHAIN_FILE:-$DEV_FAST_REPO_ROOT/tools/cranelift/VERSION}"

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

# Read a single-line version pin, trimming surrounding whitespace. A missing or
# blank pin aborts rather than yielding an empty version that would silently
# produce a nonsensical download URL or toolchain name.
read_pin() {
  local file=$1 value
  [ -f "$file" ] || fail "missing version pin: $file"
  value=$(tr -d '[:space:]' <"$file")
  [ -n "$value" ] || fail "empty version pin: $file"
  printf '%s' "$value"
}

# The pinned mold release tag, e.g. "2.41.0".
mold_version() { read_pin "$MOLD_VERSION_FILE"; }

# The pinned nightly supplying Cranelift, e.g. "nightly-2026-06-29".
cranelift_toolchain() { read_pin "$CRANELIFT_TOOLCHAIN_FILE"; }

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
