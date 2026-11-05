#!/bin/sh
# Install one built package in an emulated container and verify it.
#
# The shared validate-linux-packages action skips both its sandbox install and
# its architecture check whenever the package architecture differs from the
# runner's. That is exactly the arm64 lane on an x64 release runner, so without
# this script an arm64 package would be published having never run. The
# workflow registers QEMU binfmt handlers before building the foreign-arch
# images, so a container of the package's own architecture can execute here.
#
# Usage: install-emulated-package.sh IMAGE ARCH PACKAGE BIN_NAME
set -eu

if [ "$#" -ne 4 ]; then
  printf 'Usage: %s IMAGE ARCH PACKAGE BIN_NAME\n' "$0" >&2
  exit 64
fi

image=$1
arch=$2
package=$3
bin_name=$4

if [ ! -f "$package" ]; then
  printf 'FAIL: no package at %s\n' "$package" >&2
  exit 1
fi

case "$package" in
  *.deb) install='dpkg -i' ;;
  # The RPM image deliberately omits ninja-build, so resolution would fail for
  # a package that correctly declares it. The verifier reads the declared
  # requirements instead.
  *.rpm) install='rpm -i --nodeps --nosignature' ;;
  *)
    printf 'FAIL: %s is neither a .deb nor a .rpm\n' "$package" >&2
    exit 1
    ;;
esac

directory=$(cd "$(dirname "$package")" && pwd)
name=$(basename "$package")

# Install and verify in one container so the installed files are still present
# when the verifier runs. The image is tagged for this architecture and was
# built locally, so Podman resolves it from the local store.
podman run --rm --arch "$arch" \
  --volume "$directory:/packages:ro,z" \
  "$image" \
  sh -c "$install \"/packages/\$1\" && /usr/local/bin/verify-installed-package \"\$2\"" \
  sh "$name" "$bin_name"