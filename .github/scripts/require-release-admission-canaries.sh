#!/usr/bin/env bash
# Emit bounded release-admission scaffolding metrics while checking RFC 0005 inputs.
set -euo pipefail

# Metric names are a stable operator contract.  Counters end in `_total` and
# the latency histogram records seconds, matching ADR-013's naming convention.
readonly GATE_METRIC='netsuke_release_admission_gate_total'
readonly OPERATION_METRIC='netsuke_release_admission_operation_total'
readonly DURATION_METRIC='netsuke_release_admission_operation_duration_seconds'

readonly CANARY_HISTORY_SCAN='history_scan'
readonly CANARY_RELEASE_CANDIDATE='release_candidate'
readonly CANARY_NONE='none'

readonly OPERATION_RESOLVE_TAG_COMMIT='resolve_tag_commit'
readonly OPERATION_FETCH_CANDIDATE_REVISION='fetch_candidate_revision'
readonly OPERATION_FETCH_WORKFLOW_RUN='fetch_workflow_run'
readonly OPERATION_CHECK_SCAN_FRESHNESS='check_scan_freshness'
readonly OPERATION_VERIFY_EVIDENCE='verify_evidence'

readonly OUTCOME_SUCCESS='success'
readonly OUTCOME_FAILURE='failure'
readonly OUTCOME_UNKNOWN='unknown'

readonly ERROR_NONE='none'
readonly ERROR_API='api_error'
readonly ERROR_FETCH='fetch_error'
readonly ERROR_STALE='stale_evidence'
readonly ERROR_MISSING='missing_evidence'
readonly ERROR_MISMATCH='mismatch'
readonly ERROR_TIMEOUT='timeout'
readonly ERROR_UNKNOWN='unknown'

readonly DEFAULT_OPERATION_TIMEOUT_SECONDS=30
readonly MAX_OPERATION_TIMEOUT_SECONDS=300
readonly ADMISSION_OBSERVATION_MODE='false'
readonly ADMISSION_ENFORCEMENT_MODE='true'
readonly metrics_file="${NETSUKE_RELEASE_ADMISSION_METRICS_FILE:-${RUNNER_TEMP:-/tmp}/netsuke-release-admission-metrics.jsonl}"
readonly operation_timeout_seconds="${NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS:-$DEFAULT_OPERATION_TIMEOUT_SECONDS}"
readonly admission_enforcement="${NETSUKE_RELEASE_ADMISSION_ENFORCE-$ADMISSION_OBSERVATION_MODE}"
gate_outcome="$OUTCOME_UNKNOWN"
gate_error_category="$ERROR_UNKNOWN"
workflow_run_id=''

is_canary() {
  case "$1" in
    "$CANARY_HISTORY_SCAN"|"$CANARY_RELEASE_CANDIDATE"|"$CANARY_NONE") return 0 ;;
    *) return 1 ;;
  esac
}

is_operation() {
  case "$1" in
    "$OPERATION_RESOLVE_TAG_COMMIT"|"$OPERATION_FETCH_CANDIDATE_REVISION"|\
    "$OPERATION_FETCH_WORKFLOW_RUN"|"$OPERATION_CHECK_SCAN_FRESHNESS"|\
    "$OPERATION_VERIFY_EVIDENCE") return 0 ;;
    *) return 1 ;;
  esac
}

is_outcome() {
  case "$1" in
    "$OUTCOME_SUCCESS"|"$OUTCOME_FAILURE"|"$OUTCOME_UNKNOWN") return 0 ;;
    *) return 1 ;;
  esac
}

is_error_category() {
  case "$1" in
    "$ERROR_NONE"|"$ERROR_API"|"$ERROR_FETCH"|"$ERROR_STALE"|\
    "$ERROR_MISSING"|"$ERROR_MISMATCH"|"$ERROR_TIMEOUT"|"$ERROR_UNKNOWN") return 0 ;;
    *) return 1 ;;
  esac
}

is_metric_value() {
  [[ "$1" =~ ^[0-9]+([.][0-9]+)?$ ]]
}

is_operation_timeout() {
  [[ "$1" =~ ^[1-9][0-9]*$ && ${#1} -le 3 ]] && \
    (( 10#$1 <= MAX_OPERATION_TIMEOUT_SECONDS ))
}

is_admission_enforcement() {
  [[ "$1" == "$ADMISSION_OBSERVATION_MODE" || "$1" == "$ADMISSION_ENFORCEMENT_MODE" ]]
}

is_timeout_status() {
  [[ "$1" -eq 124 || "$1" -eq 137 ]]
}

run_bounded_command() {
  timeout --kill-after=1s "${operation_timeout_seconds}s" "$@"
}

write_unknown_metric() {
  local metric_name="$1"

  case "$metric_name" in
    "$GATE_METRIC")
      printf '%s\n' \
        '{"name":"netsuke_release_admission_gate_total","labels":{"outcome":"unknown","error_category":"unknown"},"value":1}' \
        >>"$metrics_file"
      ;;
    "$OPERATION_METRIC")
      printf '%s\n' \
        '{"name":"netsuke_release_admission_operation_total","labels":{"canary":"none","operation":"verify_evidence","outcome":"unknown","error_category":"unknown"},"value":1}' \
        >>"$metrics_file"
      ;;
    "$DURATION_METRIC")
      printf '%s\n' \
        '{"name":"netsuke_release_admission_operation_duration_seconds","labels":{"operation":"verify_evidence"},"value":0}' \
        >>"$metrics_file"
      ;;
    *)
      printf '%s\n' 'release-admission metric name is outside the fixed vocabulary' >&2
      return 1
      ;;
  esac
}

emit_metric() {
  local metric_name="$1"
  local canary="$2"
  local operation="$3"
  local outcome="$4"
  local error_category="$5"
  local value="$6"

  if ! is_canary "$canary" || ! is_operation "$operation" || ! is_outcome "$outcome" || \
    ! is_error_category "$error_category" || ! is_metric_value "$value"; then
    printf '%s\n' 'release-admission metric labels are outside the fixed vocabulary' >&2
    write_unknown_metric "$metric_name"
    return 1
  fi

  case "$metric_name" in
    "$GATE_METRIC")
      printf '{"name":"%s","labels":{"outcome":"%s","error_category":"%s"},"value":%s}\n' \
        "$metric_name" "$outcome" "$error_category" "$value" >>"$metrics_file"
      ;;
    "$OPERATION_METRIC")
      printf '{"name":"%s","labels":{"canary":"%s","operation":"%s","outcome":"%s","error_category":"%s"},"value":%s}\n' \
        "$metric_name" "$canary" "$operation" "$outcome" "$error_category" "$value" \
        >>"$metrics_file"
      ;;
    "$DURATION_METRIC")
      printf '{"name":"%s","labels":{"operation":"%s"},"value":%s}\n' \
        "$metric_name" "$operation" "$value" >>"$metrics_file"
      ;;
    *)
      printf '%s\n' 'release-admission metric name is outside the fixed vocabulary' >&2
      return 1
      ;;
  esac
}

monotonic_seconds() {
  python3 -c 'import time; print(time.monotonic())'
}

duration_seconds() {
  python3 - "$1" "$2" <<'PY'
import sys

started, finished = map(float, sys.argv[1:])
print(max(0.0, finished - started))
PY
}

record_gate_result() {
  emit_metric "$GATE_METRIC" "$CANARY_NONE" "$OPERATION_VERIFY_EVIDENCE" \
    "$gate_outcome" "$gate_error_category" 1
  {
    printf 'gate-outcome=%s\n' "$gate_outcome"
    printf 'gate-error-category=%s\n' "$gate_error_category"
    printf 'metrics-file=%s\n' "$metrics_file"
  } >>"$GITHUB_OUTPUT"
}

finish_gate() {
  record_gate_result
}

run_operation() {
  local canary="$1"
  local operation="$2"
  shift 2
  local started finished duration outcome error_category
  local operation_error_category="$ERROR_UNKNOWN"

  started="$(monotonic_seconds)"
  if "$@"; then
    outcome="$OUTCOME_SUCCESS"
    error_category="$ERROR_NONE"
  else
    outcome="$OUTCOME_FAILURE"
    error_category="$operation_error_category"
  fi
  finished="$(monotonic_seconds)"
  duration="$(duration_seconds "$started" "$finished")"
  emit_metric "$OPERATION_METRIC" "$canary" "$operation" "$outcome" \
    "$error_category" 1
  emit_metric "$DURATION_METRIC" "$CANARY_NONE" "$operation" "$OUTCOME_UNKNOWN" \
    "$ERROR_UNKNOWN" "$duration"

  if [[ "$outcome" == "$OUTCOME_FAILURE" ]]; then
    gate_outcome="$OUTCOME_FAILURE"
    gate_error_category="$error_category"
    return 1
  fi
}

resolve_tag_commit() {
  local command_status resolved_revision
  if resolved_revision="$(run_bounded_command gh api "repos/${GITHUB_REPOSITORY}/commits/${GITHUB_SHA}" --jq '.sha')"; then
    :
  else
    command_status=$?
    if is_timeout_status "$command_status"; then
      operation_error_category="$ERROR_TIMEOUT"
    else
      operation_error_category="$ERROR_API"
    fi
    return 1
  fi
  if [[ "$resolved_revision" != "$GITHUB_SHA" ]]; then
    operation_error_category="$ERROR_MISMATCH"
    return 1
  fi
}

fetch_candidate_revision() {
  local command_status
  if run_bounded_command git fetch --depth 1 --no-tags origin -- "$GITHUB_SHA"; then
    :
  else
    command_status=$?
    if is_timeout_status "$command_status"; then
      operation_error_category="$ERROR_TIMEOUT"
    else
      operation_error_category="$ERROR_FETCH"
    fi
    return 1
  fi
}

fetch_workflow_run() {
  local command_status
  if workflow_run_id="$(run_bounded_command gh api "repos/${GITHUB_REPOSITORY}/actions/runs?head_sha=${GITHUB_SHA}&per_page=1" --jq '.workflow_runs[0].id // empty')"; then
    :
  else
    command_status=$?
    if is_timeout_status "$command_status"; then
      operation_error_category="$ERROR_TIMEOUT"
    else
      operation_error_category="$ERROR_API"
    fi
    return 1
  fi
}

check_scan_freshness() {
  case "${NETSUKE_RELEASE_ADMISSION_EVIDENCE_STATE:-missing}" in
    fresh) ;;
    stale)
      operation_error_category="$ERROR_STALE"
      return 1
      ;;
    missing|'')
      operation_error_category="$ERROR_MISSING"
      return 1
      ;;
    *)
      operation_error_category="$ERROR_UNKNOWN"
      return 1
      ;;
  esac
}

verify_evidence() {
  # Freshness is not evidence: RFC 0005 still requires a producer-backed record.
  if [[ "${NETSUKE_RELEASE_ADMISSION_EVIDENCE_STATE:-missing}" == 'fresh' ]]; then
    operation_error_category="$ERROR_MISSING"
    return 1
  fi
  if [[ -z "$workflow_run_id" ]]; then
    operation_error_category="$ERROR_MISSING"
    return 1
  fi
}

run_admission_operations() {
  run_operation "$CANARY_NONE" "$OPERATION_RESOLVE_TAG_COMMIT" resolve_tag_commit || return 1
  run_operation "$CANARY_RELEASE_CANDIDATE" "$OPERATION_FETCH_CANDIDATE_REVISION" \
    fetch_candidate_revision || return 1
  run_operation "$CANARY_HISTORY_SCAN" "$OPERATION_FETCH_WORKFLOW_RUN" fetch_workflow_run || return 1
  run_operation "$CANARY_HISTORY_SCAN" "$OPERATION_CHECK_SCAN_FRESHNESS" \
    check_scan_freshness || return 1
  run_operation "$CANARY_HISTORY_SCAN" "$OPERATION_VERIFY_EVIDENCE" verify_evidence
}

mkdir -p "$(dirname "$metrics_file")"
: >"$metrics_file"
: "${GITHUB_OUTPUT:?GITHUB_OUTPUT must identify the workflow output file}"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must identify the release repository}"
: "${GITHUB_SHA:?GITHUB_SHA must identify the release candidate revision}"
trap finish_gate EXIT

if ! is_operation_timeout "$operation_timeout_seconds"; then
  printf '%s\n' 'release-admission operation timeout must be between 1 and 300 seconds' >&2
  gate_outcome="$OUTCOME_FAILURE"
  gate_error_category="$ERROR_UNKNOWN"
  exit 1
fi

if ! is_admission_enforcement "$admission_enforcement"; then
  printf '%s\n' 'release-admission enforcement must be true or false' >&2
  gate_outcome="$OUTCOME_FAILURE"
  gate_error_category="$ERROR_UNKNOWN"
  exit 1
fi

if ! run_admission_operations; then
  if [[ "$admission_enforcement" == "$ADMISSION_ENFORCEMENT_MODE" ]]; then
    exit 1
  fi
  exit 0
fi

gate_outcome="$OUTCOME_SUCCESS"
gate_error_category="$ERROR_NONE"
