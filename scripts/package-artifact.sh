#!/usr/bin/env bash
set -euo pipefail

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
cp "$bin_src" "$bin_dst"
sha256sum "$bin_dst" > "${bin_dst}.sha256"

man_src="target/generated-man/${bin_name}.1"
man_dst="$out_dir/${bin_name}-${os}-${arch}.1"
cp "$man_src" "$man_dst"
sha256sum "$man_dst" > "${man_dst}.sha256"
