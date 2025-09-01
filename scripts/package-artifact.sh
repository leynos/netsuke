#!/usr/bin/env bash
set -euo pipefail
hash256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1"
  else
    shasum -a 256 "$1"
  fi
}

require_file() {
  local path="$1"
  local title="$2"
  local hint="$3"
  if [[ ! -f "$path" ]]; then
    echo "::error title=${title}::${path} not found. ${hint}"
    exit 1
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
require_file "$bin_src" "binary missing" "Did the build succeed for target=${target}?"
cp "$bin_src" "$bin_dst"
hash256 "$bin_dst" > "${bin_dst}.sha256"

man_src="target/generated-man/${bin_name}.1"
man_dst="$out_dir/${bin_name}-${os}-${arch}.1"
require_file "$man_src" "man page missing" "Did build.rs run and write ${bin_name}.1?"
cp "$man_src" "$man_dst"
hash256 "$man_dst" > "${man_dst}.sha256"
