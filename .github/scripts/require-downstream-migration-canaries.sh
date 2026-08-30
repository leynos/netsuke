#!/usr/bin/env bash
# Require trusted downstream evidence for the exact Netsuke revision being released.
set -euo pipefail

emit_event() {
  printf 'release_admission canary=%s operation=%s outcome=%s\n' \
    "$1" "$2" "$3" >&2
}

emit_failure_event() {
  printf 'release_admission canary=%s operation=%s outcome=failure error_category=%s\n' \
    "$1" "$2" "$3" >&2
}

workflow_name="Netsuke v0.1.0 release-admission canary candidate ${GITHUB_SHA}"
while read -r canary repository revision workflow_id branch; do
  emit_event "${canary}" workflow_source_fetch started
  if ! workflow_source="$(gh api \
    "repos/${repository}/contents/.github/workflows/netsuke-canary.yml?ref=${revision}" \
    --jq '.content' | base64 --decode)"; then
    emit_failure_event "${canary}" workflow_source_fetch workflow_source_fetch_failed
    echo "Pinned canary workflow for ${repository}@${revision} could not be fetched" >&2
    exit 1
  fi
  emit_event "${canary}" workflow_source_fetch success

  emit_event "${canary}" workflow_source_validation started
  for expected_line in \
    "uses: leynos/netsuke/.github/actions/install-release-candidate@${GITHUB_SHA}" \
    "revision: ${GITHUB_SHA}"; do
    if ! grep --fixed-strings --quiet "${expected_line}" <<<"${workflow_source}"; then
      emit_failure_event "${canary}" workflow_source_validation candidate_reference_mismatch
      echo "Pinned canary workflow for ${repository}@${revision} does not test ${GITHUB_SHA}" >&2
      exit 1
    fi
  done
  if ! WORKFLOW_SOURCE="$workflow_source" python3 - "$GITHUB_SHA" <<'PY'
import os
import sys

import yaml

candidate = sys.argv[1]
workflow = yaml.safe_load(os.environ["WORKFLOW_SOURCE"])
matches = []
for job in workflow.get("jobs", {}).values() if isinstance(workflow, dict) else []:
    if not isinstance(job, dict):
        continue
    for step in job.get("steps", []):
        if not isinstance(step, dict):
            continue
        if (
            step.get("uses")
            == f"leynos/netsuke/.github/actions/install-release-candidate@{candidate}"
            and isinstance(step.get("with"), dict)
            and step["with"].get("revision") == candidate
        ):
            matches.append(step)

sys.exit(0 if len(matches) == 1 else 1)
PY
  then
    emit_failure_event "${canary}" workflow_source_validation candidate_reference_mismatch
    echo "Pinned canary workflow for ${repository}@${revision} does not test ${GITHUB_SHA}" >&2
    exit 1
  fi
  emit_event "${canary}" workflow_source_validation success

  emit_event "${canary}" workflow_run_lookup started
  if ! run_id="$(gh api \
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
      .conclusion == \"success\") | .id] | first // empty")"; then
    emit_failure_event "${canary}" workflow_run_lookup workflow_run_lookup_failed
    echo "Pinned canary workflow runs for ${repository}@${revision} could not be fetched" >&2
    exit 1
  fi
  emit_event "${canary}" workflow_run_lookup success

  emit_event "${canary}" trusted_run_validation started
  if [ -z "$run_id" ]; then
    emit_failure_event "${canary}" trusted_run_validation missing_successful_evidence
    echo "Missing successful ${workflow_name} for ${repository}@${revision}" >&2
    exit 1
  fi
  emit_event "${canary}" trusted_run_validation success
  echo "Accepted ${repository}@${revision} from run ${run_id}"
done <<'CANARIES'
repovec-appliance leynos/repovec-appliance 6be365b4b30ef48537add5719a9b387ccc41777f 343316513 issue-598-v010-netsuke-canary
mxd leynos/mxd 8146278cc82506c222bb78d4f3fc05c12ed95b41 343314513 issue-598-v010-netsuke-canary
ortho-config leynos/ortho-config b42b5d0adfacd79456d2a2f9edbf9f561aac943b 343328370 issue-598-v010-netsuke-canary
CANARIES
