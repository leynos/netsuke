#!/usr/bin/env bash
# Shared helpers for the opt-in mold + Cranelift local build acceleration.
#
# Sourced by scripts/install-dev-fast.sh and scripts/dev-fast-check.sh so both
# resolve the pinned versions and emit diagnostics identically. Every message is
# prefixed `dev-fast:` and written to stderr, matching the `prover-tools:`
# convention used by the Kani and Verus targets.

set -euo pipefail

: "${MOLD_VERSION_FILE:?MOLD_VERSION_FILE must be set}"
: "${MOLD_SHA256SUMS_FILE:?MOLD_SHA256SUMS_FILE must be set}"
: "${CRANELIFT_TOOLCHAIN_FILE:?CRANELIFT_TOOLCHAIN_FILE must be set}"

# Prefix for the mold installation tree. `bin/` is prepended to PATH by the
# Makefile, so a pinned mold shadows any older distribution package.
DEV_FAST_PREFIX="${DEV_FAST_PREFIX:-$HOME/.local}"

# shellcheck disable=SC2034 # consumed by the scripts that source this file.
CRANELIFT_COMPONENT='rustc-codegen-cranelift-preview'

note() { printf 'dev-fast: %s\n' "$*" >&2; }

fail() {
  printf 'dev-fast: %s\n' "$*" >&2
  exit 1
}

read_pin() {
  local file=$1 value
  [ -f "$file" ] || fail "missing version pin: $file"
  value=$(tr -d '[:space:]' <"$file")
  [ -n "$value" ] || fail "empty version pin: $file"
  printf '%s' "$value"
}

mold_version() { read_pin "$MOLD_VERSION_FILE"; }

cranelift_toolchain() { read_pin "$CRANELIFT_TOOLCHAIN_FILE"; }

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

installed_mold_version() {
  # `mold --version` prints e.g. "mold 2.41.0 (compatible with GNU ld)".
  mold --version 2>/dev/null | awk 'NR == 1 { print $2 }'
}

has_cranelift_component() {
  local toolchain=$1
  rustup component list --installed --toolchain "$toolchain" 2>/dev/null |
    grep -q '^rustc-codegen-cranelift'
}
