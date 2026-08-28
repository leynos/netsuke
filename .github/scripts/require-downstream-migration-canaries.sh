#!/usr/bin/env bash
# Require trusted downstream evidence for the exact Netsuke revision being released.
set -euo pipefail

workflow_name="Netsuke v0.1.0 release-admission canary candidate ${GITHUB_SHA}"
while read -r repository revision workflow_id branch; do
  workflow_source="$(gh api \
    "repos/${repository}/contents/.github/workflows/netsuke-canary.yml?ref=${revision}" \
    --jq '.content' | base64 --decode)"
  for expected_line in \
    "uses: leynos/netsuke/.github/actions/install-release-candidate@${GITHUB_SHA}" \
    "revision: ${GITHUB_SHA}"; do
    if ! grep --fixed-strings --quiet "${expected_line}" <<<"${workflow_source}"; then
      echo "Pinned canary workflow for ${repository}@${revision} does not test ${GITHUB_SHA}" >&2
      exit 1
    fi
  done

  run_id="$(gh api \
    "repos/${repository}/actions/workflows/${workflow_id}/runs?head_sha=${revision}&per_page=100" \
    --jq "[.workflow_runs[] | select(\
      .repository.full_name == \"${repository}\" and \
      .workflow_id == ${workflow_id} and \
      .path == \".github/workflows/netsuke-canary.yml\" and \
      .event == \"push\" and \
      .head_branch == \"${branch}\" and \
      .head_sha == \"${revision}\" and \
      .name == \"${workflow_name}\" and \
      .status == \"completed\" and \
      .conclusion == \"success\") | .id] | first // empty")"
  if [ -z "$run_id" ]; then
    echo "Missing successful ${workflow_name} for ${repository}@${revision}" >&2
    exit 1
  fi
  echo "Accepted ${repository}@${revision} from run ${run_id}"
done <<'CANARIES'
leynos/repovec-appliance 6be365b4b30ef48537add5719a9b387ccc41777f 343316513 issue-598-v010-netsuke-canary
leynos/mxd 8146278cc82506c222bb78d4f3fc05c12ed95b41 343314513 issue-598-v010-netsuke-canary
leynos/ortho-config b42b5d0adfacd79456d2a2f9edbf9f561aac943b 343328370 issue-598-v010-netsuke-canary
CANARIES
