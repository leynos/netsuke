#!/usr/bin/env bash
# shellcheck shell=bash
# Own the runner timestamps and GITHUB_OUTPUT fields for this composite action.
# Keep this helper private to publish-crates-io; it is not a general workflow
# timing library.

publisher_timing_start() {
  printf 'started_ns=%s\n' "$(date +%s%N)" >>"$GITHUB_OUTPUT"
}

publisher_timing_end() {
  printf 'ended_ns=%s\n' "$(date +%s%N)" >>"$GITHUB_OUTPUT"
}

publisher_timing_finish_step() {
  local step_exit_status=$?
  trap - EXIT
  publisher_timing_end
  exit "$step_exit_status"
}

publisher_timing_start_step() {
  publisher_timing_start
  trap publisher_timing_finish_step EXIT
}
