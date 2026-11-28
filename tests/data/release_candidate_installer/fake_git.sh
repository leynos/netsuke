#!/usr/bin/env bash
set -euo pipefail

if [[ "$1" == "-C" ]]; then
  shift 2
fi

printf '%q ' "$@" >> "${NETSUKE_GIT_ARGS}"
printf '\n' >> "${NETSUKE_GIT_ARGS}"

case "$1" in
  init|remote|fetch|checkout) exit 0 ;;
  rev-parse) printf '%s\n' "${NETSUKE_FAKE_RESOLVED_REVISION}" ;;
  *) echo "unexpected git invocation: $*" >&2; exit 1 ;;
esac
