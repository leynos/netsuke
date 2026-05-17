#!/usr/bin/env bash
# Verifies that the installed Kani command matches the repository pin.
set -euo pipefail

fail() {
  echo "check-kani-version: $*" >&2
  exit 1
}

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "${script_dir}/.." && pwd)"
version_file="${repo_root}/tools/kani/VERSION"
kani_command="${KANI:-cargo kani}"

[[ -f "$version_file" ]] ||
  fail "version pin '${version_file}' does not exist"

expected_version="$(tr -d '[:space:]' < "$version_file")"
[[ -n "$expected_version" ]] ||
  fail "version pin '${version_file}' is empty"
[[ "$expected_version" =~ ^[0-9]+[.][0-9]+[.][0-9]+$ ]] ||
  fail "version pin '${expected_version}' must use MAJOR.MINOR.PATCH format"

IFS=' ' read -r -a kani_args <<< "$kani_command"
((${#kani_args[@]} > 0)) ||
  fail "KANI command is empty"

version_output="$("${kani_args[@]}" --version)" ||
  fail "failed to run '${kani_command} --version'"

[[ "$version_output" =~ ([0-9]+[.][0-9]+[.][0-9]+) ]] ||
  fail "could not parse Kani version from: ${version_output}"

actual_version="${BASH_REMATCH[1]}"
[[ "$actual_version" == "$expected_version" ]] ||
  fail "expected Kani ${expected_version} from ${version_file}, found ${actual_version}"

echo "Kani ${actual_version} matches ${version_file}."
