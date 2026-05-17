#!/usr/bin/env bash
# Installs the repository-supported Kani verifier version.
set -euo pipefail

fail() {
  echo "install-kani: $*" >&2
  exit 1
}

require_command() {
  local command_name="$1"
  command -v "$command_name" >/dev/null 2>&1 ||
    fail "required command '${command_name}' was not found on PATH"
}

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"
version_file="${repo_root}/tools/kani/VERSION"

require_command cargo

[[ -f "$version_file" ]] ||
  fail "version pin '${version_file}' does not exist"

kani_version="$(tr -d '[:space:]' < "$version_file")"
[[ -n "$kani_version" ]] ||
  fail "version pin '${version_file}' is empty"
[[ "$kani_version" =~ ^[0-9]+[.][0-9]+[.][0-9]+$ ]] ||
  fail "version pin '${kani_version}' must use MAJOR.MINOR.PATCH format"

echo "Installing kani-verifier ${kani_version} from ${version_file}."
cargo install --locked kani-verifier --version "$kani_version"

echo "Running cargo kani setup for kani-verifier ${kani_version}."
cargo kani setup

echo "Verifying cargo kani is callable."
cargo kani --version
