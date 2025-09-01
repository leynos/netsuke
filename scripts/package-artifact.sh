#!/usr/bin/env bash
set -euo pipefail
hash256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1"
  else
    shasum -a 256 "$1"
  fi
}

# package-artifact.sh <os> <arch> <target> <ext> <bin_name>
# Copies the built binary and generated man page into an OS/arch-specific
# directory and writes SHA256 checksums for each file.

os="$1"
arch="$2"
target="$3"
ext="$4"
bin_name="$5"

out_dir="artifacts/${os}-${arch}"
mkdir -p "$out_dir"

bin_src="target/${target}/release/${bin_name}${ext}"
bin_dst="$out_dir/${bin_name}-${os}-${arch}${ext}"
if [[ ! -f "$bin_src" ]]; then
  echo "::error title=binary missing::${bin_src} not found. Did the build succeed for target=${target}?"
  exit 1
fi
cp "$bin_src" "$bin_dst"
hash256 "$bin_dst" > "${bin_dst}.sha256"

man_src="target/generated-man/${bin_name}.1"
man_dst="$out_dir/${bin_name}-${os}-${arch}.1"
if [[ ! -f "$man_src" ]]; then
  echo "::error title=man page missing::${man_src} not found. Did build.rs run and write ${bin_name}.1?"
  exit 1
fi
cp "$man_src" "$man_dst"
hash256 "$man_dst" > "${man_dst}.sha256"
