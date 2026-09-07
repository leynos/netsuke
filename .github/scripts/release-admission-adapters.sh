#!/usr/bin/env bash
# Provide explicit external-effect adapters for release-admission orchestration.

run_bounded_command() {
  local timeout_seconds="$1"
  shift
  timeout --kill-after=1s "${timeout_seconds}s" "$@"
}

github_resolve_commit() {
  local adapter="$1" timeout_seconds="$2" repository="$3" revision="$4"
  run_bounded_command "$timeout_seconds" "$adapter" api \
    "repos/$repository/commits/$revision" --jq '.sha'
}

github_find_workflow_run() {
  local adapter="$1" timeout_seconds="$2" repository="$3" revision="$4"
  run_bounded_command "$timeout_seconds" "$adapter" api \
    "repos/$repository/actions/runs?head_sha=$revision&per_page=1" \
    --jq '.workflow_runs[0].id // empty'
}

git_fetch_revision() {
  local adapter="$1" timeout_seconds="$2" revision="$3"
  run_bounded_command "$timeout_seconds" "$adapter" fetch --depth 1 --no-tags \
    origin -- "$revision"
}

monotonic_seconds() {
  local adapter="$1"
  "$adapter" -c 'import time; print(time.monotonic())'
}

duration_seconds() {
  python3 - "$1" "$2" <<'PY'
import sys
started, finished = map(float, sys.argv[1:])
print(max(0.0, finished - started))
PY
}

append_record() {
  local adapter="$1" file="$2" record="$3"
  if [[ -n "$adapter" ]]; then
    printf '%s\n' "$record" | "$adapter" "$file"
  else
    printf '%s\n' "$record" >>"$file"
  fi
}

write_metric_record() {
  append_record "$1" "$2" "$3"
}

write_workflow_output() {
  local adapter="$1" output_file="$2" record="$3"
  if [[ -n "$adapter" ]]; then
    printf '%s\n' "$record" | "$adapter" "$output_file"
  else
    printf '%s\n' "$record" >>"$output_file"
  fi
}
