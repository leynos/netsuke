#!/usr/bin/env bash
# Install the pinned toolchain for the opt-in mold + Cranelift local build path.
#
# Downloads the pinned mold release, verifies it against tools/mold/SHA256SUMS,
# unpacks it under $DEV_FAST_PREFIX (default ~/.local), then installs the pinned
# nightly toolchain and its Cranelift codegen backend. Nothing here touches the
# release, packaging, coverage, or formal-verification toolchains.

set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=scripts/dev-fast-common.sh
. "$script_dir/dev-fast-common.sh"

MOLD_RELEASE_BASE_URL=${MOLD_RELEASE_BASE_URL:-https://github.com/rui314/mold/releases/download}

# The scratch directory the EXIT trap removes. A trap whose action is a string
# is re-parsed by the shell when it fires, so a path containing a quote — from a
# quote-bearing `TMPDIR`, say — breaks the quoting and the cleanup never runs.
# Naming a function instead means the path is only ever a variable, expanded at
# removal time and never re-parsed. It is script-scope rather than local because
# the trap fires after `install_mold` has returned.
DEV_FAST_WORKDIR=

remove_workdir() {
  [ -n "$DEV_FAST_WORKDIR" ] || return 0
  rm -rf -- "$DEV_FAST_WORKDIR"
  DEV_FAST_WORKDIR=
}

trap remove_workdir EXIT

# Verify the downloaded tarball against the single matching line in
# SHA256SUMS. An unlisted artefact is a hard failure, never a silent skip.
verify_mold_archive() {
  local archive=$1 name=$2 expected recorded
  expected=$(awk -v name="$name" '$2 == name { print $1 }' "$MOLD_SHA256SUMS_FILE")
  [ -n "$expected" ] || fail "no checksum recorded for $name in $MOLD_SHA256SUMS_FILE"
  # Refuse an ambiguous file rather than guessing. Several rows for one artefact
  # make `expected` multi-line, and the check below would then hand `sha256sum`
  # one malformed line per extra digest plus a single well-formed one. Malformed
  # lines are only warned about, so the verdict would silently rest on whichever
  # digest happened to come last — a file recording a wrong digest alongside the
  # right one would verify.
  recorded=$(printf '%s\n' "$expected" | grep -c .)
  [ "$recorded" -eq 1 ] ||
    fail "$recorded checksums recorded for $name in $MOLD_SHA256SUMS_FILE; refusing to guess"
  printf '%s  %s\n' "$expected" "$archive" | sha256sum --check --status ||
    fail "checksum mismatch for $name; refusing to install"
  note "verified $name against $MOLD_SHA256SUMS_FILE"
}

# Download, verify, and unpack the pinned mold release, or explain why the step
# is being skipped on a platform mold does not support.
install_mold() {
  local version=$1 arch name url workdir
  if ! is_linux; then
    note "mold is Linux-only; skipping on $(uname -s), the platform linker is used instead"
    return 0
  fi
  arch=$(mold_arch)
  name="mold-$version-$arch-linux.tar.gz"
  url="$MOLD_RELEASE_BASE_URL/v$version/$name"

  DEV_FAST_WORKDIR=$(mktemp -d)
  workdir=$DEV_FAST_WORKDIR

  note "downloading $url"
  curl --fail --silent --show-error --location --output "$workdir/$name" "$url" ||
    fail "failed to download $name"
  verify_mold_archive "$workdir/$name" "$name"

  # The tarball root is mold-<version>-<arch>-linux/{bin,lib,libexec}; strip it
  # so the tree merges into the prefix and `bin/ld.mold` lands on PATH.
  mkdir -p "$DEV_FAST_PREFIX"
  tar --extract --gzip --strip-components=1 --directory "$DEV_FAST_PREFIX" --file "$workdir/$name" ||
    fail "failed to unpack $name into $DEV_FAST_PREFIX"
  note "installed mold $version into $DEV_FAST_PREFIX"
  # The `make dev-*` recipes prepend this prefix to PATH themselves; the hint
  # matters only when the scripts are invoked directly.
  note "put $DEV_FAST_PREFIX/bin first on PATH when not using the make targets"
}

# Install the pinned nightly and its Cranelift backend component. Uses the
# minimal profile: this toolchain exists to supply a codegen backend, not to
# replace the repository's stable toolchain.
install_cranelift() {
  local toolchain=$1
  command -v rustup >/dev/null 2>&1 ||
    fail 'rustup not found on PATH; install it from https://rustup.rs'
  note "installing toolchain $toolchain"
  rustup toolchain install "$toolchain" --profile minimal ||
    fail "failed to install toolchain $toolchain"
  note "installing $CRANELIFT_COMPONENT for $toolchain"
  rustup component add "$CRANELIFT_COMPONENT" --toolchain "$toolchain" ||
    fail "failed to install $CRANELIFT_COMPONENT for $toolchain"
}

# Install both halves. The linker step runs first so a checksum failure aborts
# before spending time on a toolchain download.
main() {
  local mold_pin toolchain_pin
  # Resolve the pins into variables first: `fail` exits, but inside a command
  # substitution that exit kills only the subshell, so an unreadable pin would
  # otherwise reach the installer as an empty string and be built into a
  # download URL.
  mold_pin=$(mold_version) || return 1
  toolchain_pin=$(cranelift_toolchain) || return 1
  install_mold "$mold_pin"
  install_cranelift "$toolchain_pin"
  note 'ready; verify with: make dev-fast-check'
}

main "$@"
