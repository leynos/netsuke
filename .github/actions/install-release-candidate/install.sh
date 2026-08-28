#!/usr/bin/env bash
# Build and expose the exact Netsuke candidate requested by a downstream canary.
set -euo pipefail

candidate_root="$(mktemp -d "${RUNNER_TEMP}/netsuke-candidate-XXXXXX")"
candidate_bin_dir="${candidate_root}/bin"
candidate_source_dir="${candidate_root}/source"
candidate_binary_name=netsuke
if [[ "${RUNNER_OS}" == "Windows" ]]; then
  candidate_binary_name=netsuke.exe
fi
candidate_binary="${candidate_bin_dir}/${candidate_binary_name}"

mkdir -p "${candidate_source_dir}" "${candidate_bin_dir}"
git -C "${candidate_source_dir}" init --quiet
git -C "${candidate_source_dir}" remote add origin https://github.com/leynos/netsuke.git
git -C "${candidate_source_dir}" fetch --depth 1 origin "${NETSUKE_CANDIDATE_REVISION}"
git -C "${candidate_source_dir}" checkout --detach --quiet FETCH_HEAD

resolved_revision="$(git -C "${candidate_source_dir}" rev-parse HEAD)"
if [[ "${resolved_revision}" != "${NETSUKE_CANDIDATE_REVISION}" ]]; then
  echo "candidate revision mismatch: expected ${NETSUKE_CANDIDATE_REVISION}, got ${resolved_revision}" >&2
  exit 1
fi

(
  cd "${candidate_source_dir}"
  cargo build --locked --release --bin netsuke
)
cp "${candidate_source_dir}/target/release/${candidate_binary_name}" "${candidate_binary}"

version_output="$("${candidate_binary}" --version)"
resolved_version="${version_output#netsuke }"
if [[ "${resolved_version}" != "${NETSUKE_CANDIDATE_VERSION}" ]]; then
  echo "candidate version mismatch: expected ${NETSUKE_CANDIDATE_VERSION}, got ${resolved_version}" >&2
  exit 1
fi

{
  echo "binary=${candidate_binary}"
  echo "revision=${resolved_revision}"
  echo "version=${resolved_version}"
} >> "${GITHUB_OUTPUT}"
