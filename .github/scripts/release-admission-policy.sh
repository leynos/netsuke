#!/usr/bin/env bash
# Produce bounded, side-effect-free release-admission policy decisions.

policy_success() {
  printf '%s\t%s\n' 'success' 'none'
}

policy_command_failure() {
  local status="$1" error_category="$2"
  case "$status" in
    124|137) printf '%s\t%s\n' 'failure' 'timeout' ;;
    *) printf '%s\t%s\n' 'failure' "$error_category" ;;
  esac
}

policy_commit_resolution() {
  local revision="$1" resolved_revision="$2"
  if [[ "$resolved_revision" == "$revision" ]]; then
    policy_success
  else
    printf '%s\t%s\n' 'failure' 'mismatch'
  fi
}

policy_scan_freshness() {
  case "$1" in
    fresh) policy_success ;;
    stale) printf '%s\t%s\n' 'failure' 'stale_evidence' ;;
    missing|'') printf '%s\t%s\n' 'failure' 'missing_evidence' ;;
    *) printf '%s\t%s\n' 'failure' 'unknown' ;;
  esac
}

policy_evidence() {
  local state="$1" workflow_run_id="$2"
  if [[ "$state" == fresh || -z "$workflow_run_id" ]]; then
    printf '%s\t%s\n' 'failure' 'missing_evidence'
  else
    policy_success
  fi
}
