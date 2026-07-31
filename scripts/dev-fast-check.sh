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

# Report on the linker half of the prerequisites. Returns non-zero only when
# mold is required but unusable; a non-Linux host and a version drift from the
# pin are both tolerated, with a note explaining what will happen instead.
check_mold() {
  local pinned=$1 installed resolved
  if ! is_linux; then
    note "mold is Linux-only; falling back to the default $(uname -s) linker"
    return 0
  fi
  if ! resolved=$(command -v mold 2>/dev/null); then
    note "mold not found on PATH (pinned $pinned)"
    note 'install it with: make install-dev-fast'
    return 1
  fi
  # A mold that cannot report its version is broken — a truncated download or
  # an unresolved shared library — so treat it as a failure rather than letting
  # the empty string surface as a confusing version-drift warning.
  if ! installed=$(installed_mold_version) || [ -z "$installed" ]; then
    note "mold at $resolved is on PATH but cannot report its version"
    note 'reinstall it with: make install-dev-fast'
    return 1
  fi
  # Report the resolved path, not just the version: `-fuse-ld=mold` selects by
  # PATH order, so naming the winner makes an unexpected pick obvious.
  if [ "$installed" != "$pinned" ]; then
    note "mold $installed at $resolved, pin is $pinned; run make install-dev-fast to match"
  else
    note "mold $installed at $resolved"
  fi
}

# Report on the toolchain half of the prerequisites: rustup itself, the pinned
# nightly, and the Cranelift backend component. Any absence is fatal, because
# there is no meaningful fallback for a missing codegen backend.
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

# Run both checks unconditionally so a developer sees every missing piece in one
# pass rather than fixing them one failed run at a time.
main() {
  local status=0 mold_pin toolchain_pin
  # Resolve the pins into variables first. `fail` exits, but inside a command
  # substitution that exit kills only the subshell, so passing `$(mold_version)`
  # straight into a check would continue with an empty pin and report a
  # nonsensical drift. An assignment propagates the status, so this stops.
  mold_pin=$(mold_version) || return 1
  toolchain_pin=$(cranelift_toolchain) || return 1
  check_mold "$mold_pin" || status=1
  check_cranelift "$toolchain_pin" || status=1
  [ "$status" -eq 0 ] || note 'capability check failed; see the messages above'
  return "$status"
}

main "$@"
