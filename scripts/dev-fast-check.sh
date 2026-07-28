#!/usr/bin/env bash
# Fast capability check for the opt-in mold + Cranelift local build path.
#
# Runs before `make dev-build` and `make dev-test` so a missing tool produces an
# actionable installation hint rather than an opaque codegen-backend or linker
# failure deep inside a Cargo invocation. Exits non-zero when a required
# component is absent; a version drift from the pin is reported but tolerated.

set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=scripts/dev-fast-common.sh
. "$script_dir/dev-fast-common.sh"

check_mold() {
  local pinned=$1 installed
  if ! is_linux; then
    note "mold is Linux-only; falling back to the default $(uname -s) linker"
    return 0
  fi
  if ! command -v mold >/dev/null 2>&1; then
    note "mold not found on PATH (pinned $pinned)"
    note 'install it with: make install-dev-fast'
    return 1
  fi
  installed=$(installed_mold_version)
  if [ "$installed" != "$pinned" ]; then
    note "mold $installed found, pin is $pinned; run make install-dev-fast to match"
  else
    note "mold $installed"
  fi
}

check_cranelift() {
  local toolchain=$1
  if ! command -v rustup >/dev/null 2>&1; then
    note 'rustup not found on PATH; it is required to select the pinned nightly'
    note 'install it from https://rustup.rs'
    return 1
  fi
  if ! rustup toolchain list | grep -q "^$toolchain"; then
    note "toolchain $toolchain is not installed"
    note 'install it with: make install-dev-fast'
    return 1
  fi
  if ! has_cranelift_component "$toolchain"; then
    note "$CRANELIFT_COMPONENT is not installed for $toolchain"
    note 'install it with: make install-dev-fast'
    return 1
  fi
  note "$CRANELIFT_COMPONENT available on $toolchain"
}

main() {
  local status=0
  check_mold "$(mold_version)" || status=1
  check_cranelift "$(cranelift_toolchain)" || status=1
  [ "$status" -eq 0 ] || note 'capability check failed; see the messages above'
  return "$status"
}

main "$@"
