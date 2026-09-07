#!/usr/bin/env bash
# Emit bounded release-admission observations while checking RFC 0005 inputs.
set -euo pipefail

readonly script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=release-admission-adapters.sh
source "$script_directory/release-admission-adapters.sh"
# shellcheck source=release-admission-policy.sh
source "$script_directory/release-admission-policy.sh"

# These names and value sets are a stable, bounded operator contract.
readonly GATE_METRIC='netsuke_release_admission_gate_total'
readonly OPERATION_METRIC='netsuke_release_admission_operation_total'
readonly DURATION_METRIC='netsuke_release_admission_operation_duration_seconds'
readonly TRACE_OPERATION='operation_complete'
readonly TRACE_GATE='gate_complete'
readonly TRACE_WORKFLOW_OUTPUT='workflow_output_delivery'
readonly TRACE_DELIVERY='trace_delivery'

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

# Adapter contracts: API and Git adapters accept native gh/git arguments;
# clock writes one finite seconds value; sinks accept a file path and JSONL stdin.
readonly github_api_adapter="${NETSUKE_RELEASE_ADMISSION_GH_ADAPTER:-gh}"
readonly git_fetch_adapter="${NETSUKE_RELEASE_ADMISSION_GIT_ADAPTER:-git}"
readonly clock_adapter="${NETSUKE_RELEASE_ADMISSION_CLOCK_ADAPTER:-python3}"
readonly metrics_sink_adapter="${NETSUKE_RELEASE_ADMISSION_METRICS_SINK:-}"
readonly workflow_output_sink_adapter="${NETSUKE_RELEASE_ADMISSION_OUTPUT_SINK:-}"
readonly trace_sink_adapter="${NETSUKE_RELEASE_ADMISSION_TRACE_SINK:-}"
readonly metrics_file="${NETSUKE_RELEASE_ADMISSION_METRICS_FILE:-${RUNNER_TEMP:-/tmp}/netsuke-release-admission-metrics.jsonl}"
readonly trace_file="${NETSUKE_RELEASE_ADMISSION_TRACE_FILE:-${RUNNER_TEMP:-/tmp}/netsuke-release-admission-traces.jsonl}"
readonly operation_timeout_seconds="${NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS:-$DEFAULT_OPERATION_TIMEOUT_SECONDS}"
readonly admission_enforcement="${NETSUKE_RELEASE_ADMISSION_ENFORCE-$ADMISSION_OBSERVATION_MODE}"
readonly admission_repository="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY must identify the release repository}"
readonly candidate_revision="${GITHUB_SHA:?GITHUB_SHA must identify the release candidate revision}"
readonly evidence_state="${NETSUKE_RELEASE_ADMISSION_EVIDENCE_STATE:-missing}"

gate_outcome="$OUTCOME_UNKNOWN"
gate_error_category="$ERROR_UNKNOWN"
workflow_run_id=''
trace_sink_failed=false
operation_result_operation=''
operation_result_outcome="$OUTCOME_UNKNOWN"
operation_result_error_category="$ERROR_UNKNOWN"
operation_result_duration_seconds=0

is_canary() { case "$1" in "$CANARY_HISTORY_SCAN"|"$CANARY_RELEASE_CANDIDATE"|"$CANARY_NONE") return 0;; *) return 1;; esac; }
is_operation() { case "$1" in "$OPERATION_RESOLVE_TAG_COMMIT"|"$OPERATION_FETCH_CANDIDATE_REVISION"|"$OPERATION_FETCH_WORKFLOW_RUN"|"$OPERATION_CHECK_SCAN_FRESHNESS"|"$OPERATION_VERIFY_EVIDENCE") return 0;; *) return 1;; esac; }
is_outcome() { case "$1" in "$OUTCOME_SUCCESS"|"$OUTCOME_FAILURE"|"$OUTCOME_UNKNOWN") return 0;; *) return 1;; esac; }
is_error_category() { case "$1" in "$ERROR_NONE"|"$ERROR_API"|"$ERROR_FETCH"|"$ERROR_STALE"|"$ERROR_MISSING"|"$ERROR_MISMATCH"|"$ERROR_TIMEOUT"|"$ERROR_UNKNOWN") return 0;; *) return 1;; esac; }
is_trace_event() { case "$1" in "$TRACE_OPERATION"|"$TRACE_GATE"|"$TRACE_WORKFLOW_OUTPUT"|"$TRACE_DELIVERY") return 0;; *) return 1;; esac; }
is_metric_value() { [[ "$1" =~ ^[0-9]+([.][0-9]+)?$ ]]; }
is_operation_timeout() { [[ "$1" =~ ^[1-9][0-9]*$ && ${#1} -le 3 ]] && (( 10#$1 <= MAX_OPERATION_TIMEOUT_SECONDS )); }
is_admission_enforcement() { [[ "$1" == "$ADMISSION_OBSERVATION_MODE" || "$1" == "$ADMISSION_ENFORCEMENT_MODE" ]]; }
emit_trace() {
  local event="$1" operation="$2" outcome="$3" error_category="$4" duration="$5" record
  if ! is_trace_event "$event" || ! is_operation "$operation" || ! is_outcome "$outcome" || ! is_error_category "$error_category" || ! is_metric_value "$duration"; then
    printf '%s\n' 'release-admission trace values are outside the fixed vocabulary' >&2; trace_sink_failed=true; return 0
  fi
  record=$(printf '{"event":"%s","operation":"%s","outcome":"%s","error_category":"%s","duration_seconds":%s}' "$event" "$operation" "$outcome" "$error_category" "$duration")
  if ! append_record "$trace_sink_adapter" "$trace_file" "$record"; then
    # Trace delivery is deliberately fail-open: metrics and gate results survive.
    printf '%s\n' 'release-admission trace sink failed' >&2; trace_sink_failed=true
  fi
}

emit_metric() {
  local metric_name="$1" canary="$2" operation="$3" outcome="$4" error_category="$5" value="$6" record
  if ! is_canary "$canary" || ! is_operation "$operation" || ! is_outcome "$outcome" || ! is_error_category "$error_category" || ! is_metric_value "$value"; then printf '%s\n' 'release-admission metric labels are outside the fixed vocabulary' >&2; return 1; fi
  case "$metric_name" in
    "$GATE_METRIC") record=$(printf '{"name":"%s","labels":{"outcome":"%s","error_category":"%s"},"value":%s}' "$metric_name" "$outcome" "$error_category" "$value");;
    "$OPERATION_METRIC") record=$(printf '{"name":"%s","labels":{"canary":"%s","operation":"%s","outcome":"%s","error_category":"%s"},"value":%s}' "$metric_name" "$canary" "$operation" "$outcome" "$error_category" "$value");;
    "$DURATION_METRIC") record=$(printf '{"name":"%s","labels":{"operation":"%s"},"value":%s}' "$metric_name" "$operation" "$value");;
    *) printf '%s\n' 'release-admission metric name is outside the fixed vocabulary' >&2; return 1;;
  esac
  write_metric_record "$metrics_sink_adapter" "$metrics_file" "$record"
}

record_gate_result() {
  emit_metric "$GATE_METRIC" "$CANARY_NONE" "$OPERATION_VERIFY_EVIDENCE" "$gate_outcome" "$gate_error_category" 1
  emit_trace "$TRACE_GATE" "$OPERATION_VERIFY_EVIDENCE" "$gate_outcome" "$gate_error_category" 0
  write_workflow_output "$workflow_output_sink_adapter" "$GITHUB_OUTPUT" "gate-outcome=$gate_outcome"
  write_workflow_output "$workflow_output_sink_adapter" "$GITHUB_OUTPUT" "gate-error-category=$gate_error_category"
  write_workflow_output "$workflow_output_sink_adapter" "$GITHUB_OUTPUT" "metrics-file=$metrics_file"
  write_workflow_output "$workflow_output_sink_adapter" "$GITHUB_OUTPUT" "trace-file=$trace_file"
  emit_trace "$TRACE_WORKFLOW_OUTPUT" "$OPERATION_VERIFY_EVIDENCE" "$OUTCOME_SUCCESS" "$ERROR_NONE" 0
  if [[ "$trace_sink_failed" == true ]]; then emit_trace "$TRACE_DELIVERY" "$OPERATION_VERIFY_EVIDENCE" "$OUTCOME_FAILURE" "$ERROR_UNKNOWN" 0; else emit_trace "$TRACE_DELIVERY" "$OPERATION_VERIFY_EVIDENCE" "$OUTCOME_SUCCESS" "$ERROR_NONE" 0; fi
}
finish_gate() { record_gate_result; }

run_operation() {
  local canary="$1" operation="$2"; shift 2
  local started='' finished='' clock_failed=false
  operation_result_operation="$operation"
  operation_result_outcome="$OUTCOME_UNKNOWN"
  operation_result_error_category="$ERROR_UNKNOWN"
  if ! started="$(monotonic_seconds "$clock_adapter")"; then clock_failed=true; fi
  "$@" || :
  if ! finished="$(monotonic_seconds "$clock_adapter")"; then clock_failed=true; fi
  if [[ "$clock_failed" == true ]]; then
    operation_result_outcome="$OUTCOME_FAILURE"; operation_result_error_category="$ERROR_UNKNOWN"; operation_result_duration_seconds=0
  else
    operation_result_duration_seconds="$(duration_seconds "$started" "$finished")"
  fi
  emit_metric "$OPERATION_METRIC" "$canary" "$operation" "$operation_result_outcome" "$operation_result_error_category" 1
  emit_metric "$DURATION_METRIC" "$CANARY_NONE" "$operation" "$OUTCOME_UNKNOWN" "$ERROR_UNKNOWN" "$operation_result_duration_seconds"
  emit_trace "$TRACE_OPERATION" "$operation" "$operation_result_outcome" "$operation_result_error_category" "$operation_result_duration_seconds"
  if [[ "$operation_result_outcome" != "$OUTCOME_SUCCESS" ]]; then
    gate_outcome="$OUTCOME_FAILURE"
    gate_error_category="$operation_result_error_category"
    return 1
  fi
}

set_operation_policy_result() {
  local result
  result="$("$@")"
  IFS=$'\t' read -r operation_result_outcome operation_result_error_category <<<"$result"
  if ! is_outcome "$operation_result_outcome" || ! is_error_category "$operation_result_error_category"; then
    operation_result_outcome="$OUTCOME_FAILURE"
    operation_result_error_category="$ERROR_UNKNOWN"
  fi
}

resolve_tag_commit() {
  local repository="$1" revision="$2" resolved_revision command_status
  if resolved_revision="$(github_resolve_commit "$github_api_adapter" "$operation_timeout_seconds" "$repository" "$revision")"; then
    set_operation_policy_result policy_commit_resolution "$revision" "$resolved_revision"
  else
    command_status=$?
    set_operation_policy_result policy_command_failure "$command_status" "$ERROR_API"
  fi
  [[ "$operation_result_outcome" == "$OUTCOME_SUCCESS" ]]
}
fetch_candidate_revision() {
  local revision="$1" command_status
  if git_fetch_revision "$git_fetch_adapter" "$operation_timeout_seconds" "$revision"; then
    set_operation_policy_result policy_success
  else
    command_status=$?
    set_operation_policy_result policy_command_failure "$command_status" "$ERROR_FETCH"
  fi
  [[ "$operation_result_outcome" == "$OUTCOME_SUCCESS" ]]
}
fetch_workflow_run() {
  local repository="$1" revision="$2" command_status
  if workflow_run_id="$(github_find_workflow_run "$github_api_adapter" "$operation_timeout_seconds" "$repository" "$revision")"; then
    set_operation_policy_result policy_success
  else
    command_status=$?
    set_operation_policy_result policy_command_failure "$command_status" "$ERROR_API"
  fi
  [[ "$operation_result_outcome" == "$OUTCOME_SUCCESS" ]]
}
check_scan_freshness() {
  set_operation_policy_result policy_scan_freshness "$1"
  [[ "$operation_result_outcome" == "$OUTCOME_SUCCESS" ]]
}
verify_evidence() {
  set_operation_policy_result policy_evidence "$1" "$2"
  [[ "$operation_result_outcome" == "$OUTCOME_SUCCESS" ]]
}
run_admission_operations() {
  run_operation "$CANARY_NONE" "$OPERATION_RESOLVE_TAG_COMMIT" resolve_tag_commit "$admission_repository" "$candidate_revision" || return 1
  run_operation "$CANARY_RELEASE_CANDIDATE" "$OPERATION_FETCH_CANDIDATE_REVISION" fetch_candidate_revision "$candidate_revision" || return 1
  run_operation "$CANARY_HISTORY_SCAN" "$OPERATION_FETCH_WORKFLOW_RUN" fetch_workflow_run "$admission_repository" "$candidate_revision" || return 1
  run_operation "$CANARY_HISTORY_SCAN" "$OPERATION_CHECK_SCAN_FRESHNESS" check_scan_freshness "$evidence_state" || return 1
  run_operation "$CANARY_HISTORY_SCAN" "$OPERATION_VERIFY_EVIDENCE" verify_evidence "$evidence_state" "$workflow_run_id"
}

mkdir -p "$(dirname "$metrics_file")" "$(dirname "$trace_file")"
: >"$metrics_file"; : >"$trace_file"
: "${GITHUB_OUTPUT:?GITHUB_OUTPUT must identify the workflow output file}"
trap finish_gate EXIT
if ! is_operation_timeout "$operation_timeout_seconds"; then gate_outcome="$OUTCOME_FAILURE"; gate_error_category="$ERROR_UNKNOWN"; exit 1; fi
if ! is_admission_enforcement "$admission_enforcement"; then gate_outcome="$OUTCOME_FAILURE"; gate_error_category="$ERROR_UNKNOWN"; exit 1; fi
if ! run_admission_operations; then [[ "$admission_enforcement" == "$ADMISSION_ENFORCEMENT_MODE" ]] && exit 1; exit 0; fi
gate_outcome="$OUTCOME_SUCCESS"; gate_error_category="$ERROR_NONE"
