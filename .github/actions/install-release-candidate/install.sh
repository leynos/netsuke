#!/usr/bin/env bash
# Build and expose the exact Netsuke candidate requested by a downstream canary.
set -euo pipefail

emit_event() {
  printf 'release_candidate operation=%s outcome=%s\n' "$1" "$2" >&2
}

emit_failure_event() {
  printf 'release_candidate operation=%s outcome=failure error_category=%s\n' \
    "$1" "$2" >&2
}

candidate_revision="${NETSUKE_CANDIDATE_REVISION:-}"
emit_event candidate_revision_validation started
if [[ ! "${candidate_revision}" =~ ^[[:xdigit:]]{40}$ ]]; then
  emit_failure_event candidate_revision_validation invalid_candidate_revision
  echo 'candidate revision must be a full 40-character hexadecimal commit' >&2
  exit 1
fi
emit_event candidate_revision_validation success

candidate_root="$(mktemp -d "${RUNNER_TEMP}/netsuke-candidate-XXXXXX")"
candidate_bin_dir="${candidate_root}/bin"
candidate_source_dir="${candidate_root}/source"
candidate_binary_name=netsuke
if [[ "${RUNNER_OS}" == "Windows" ]]; then
  candidate_binary_name=netsuke.exe
fi
candidate_binary="${candidate_bin_dir}/${candidate_binary_name}"

mkdir -p "${candidate_source_dir}" "${candidate_bin_dir}"
emit_event git_fetch started
if ! {
  git -C "${candidate_source_dir}" init --quiet
  git -C "${candidate_source_dir}" remote add origin https://github.com/leynos/netsuke.git
  git -C "${candidate_source_dir}" fetch --depth 1 origin -- "${candidate_revision}"
}; then
  emit_failure_event git_fetch git_fetch_failed
  echo 'candidate source fetch failed' >&2
  exit 1
fi
emit_event git_fetch success

emit_event candidate_revision_verification started
if ! git -C "${candidate_source_dir}" checkout --detach --quiet FETCH_HEAD; then
  emit_failure_event candidate_revision_verification git_checkout_failed
  echo 'candidate revision checkout failed' >&2
  exit 1
fi
if ! resolved_revision="$(git -C "${candidate_source_dir}" rev-parse HEAD)"; then
  emit_failure_event candidate_revision_verification revision_resolution_failed
  echo 'candidate revision verification failed' >&2
  exit 1
fi
if [[ "${resolved_revision}" != "${candidate_revision}" ]]; then
  emit_failure_event candidate_revision_verification revision_mismatch
  echo "candidate revision mismatch: expected ${candidate_revision}, got ${resolved_revision}" >&2
  exit 1
fi
emit_event candidate_revision_verification success

emit_event locked_cargo_build started
if ! (
  cd "${candidate_source_dir}"
  cargo build --locked --release --bin netsuke
); then
  emit_failure_event locked_cargo_build cargo_build_failed
  echo 'candidate Cargo build failed' >&2
  exit 1
fi
emit_event locked_cargo_build success
cp "${candidate_source_dir}/target/release/${candidate_binary_name}" "${candidate_binary}"

emit_event candidate_version_check started
if ! version_output="$("${candidate_binary}" --version)"; then
  emit_failure_event candidate_version_check binary_version_check_failed
  echo 'candidate binary version check failed' >&2
  exit 1
fi
resolved_version="${version_output#netsuke }"
if [[ "${resolved_version}" != "${NETSUKE_CANDIDATE_VERSION}" ]]; then
  emit_failure_event candidate_version_check candidate_version_mismatch
  echo "candidate version mismatch: expected ${NETSUKE_CANDIDATE_VERSION}, got ${resolved_version}" >&2
  exit 1
fi
emit_event candidate_version_check success

{
  echo "binary=${candidate_binary}"
  echo "revision=${resolved_revision}"
  echo "version=${resolved_version}"
} >> "${GITHUB_OUTPUT}"
