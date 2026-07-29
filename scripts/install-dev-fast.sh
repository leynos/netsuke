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

# Verify the downloaded tarball against the single matching line in
# SHA256SUMS. An unlisted artefact is a hard failure, never a silent skip.
verify_mold_archive() {
  local archive=$1 name=$2 expected
  expected=$(awk -v name="$name" '$2 == name { print $1 }' "$MOLD_SHA256SUMS_FILE")
  [ -n "$expected" ] || fail "no checksum recorded for $name in $MOLD_SHA256SUMS_FILE"
  printf '%s  %s\n' "$expected" "$archive" | sha256sum --check --status ||
    fail "checksum mismatch for $name; refusing to install"
  note "verified $name against $MOLD_SHA256SUMS_FILE"
}

install_mold() {
  local version=$1 arch name url workdir
  if ! is_linux; then
    note "mold is Linux-only; skipping on $(uname -s), the platform linker is used instead"
    return 0
  fi
  arch=$(mold_arch)
  name="mold-$version-$arch-linux.tar.gz"
  url="$MOLD_RELEASE_BASE_URL/v$version/$name"

  workdir=$(mktemp -d)
  # shellcheck disable=SC2064 # expand workdir now so the trap cannot lose it.
  trap "rm -rf '$workdir'" EXIT

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

main() {
  install_mold "$(mold_version)"
  install_cranelift "$(cranelift_toolchain)"
  note 'ready; verify with: make dev-fast-check'
}

main "$@"
