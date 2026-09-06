"""Provide isolated executable fakes for release-admission subprocess tests."""

import typing as typ

if typ.TYPE_CHECKING:
    from pathlib import Path

FAKE_ADAPTER_PREAMBLE = """#!/usr/bin/env bash
set -euo pipefail
python3 - "$NETSUKE_ADMISSION_CALL_LOG" ADAPTER_NAME "$@" <<'PY'
import json
import os
import sys

with open(sys.argv[1], "a", encoding="utf-8") as call_log:
    json.dump(
        {
            "command": sys.argv[2],
            "arguments": sys.argv[3:],
            "diagnostics": {
                "path": os.environ.get("NETSUKE_FAKE_PATH", ""),
                "url": os.environ.get("NETSUKE_FAKE_URL", ""),
            },
        },
        call_log,
    )
    call_log.write("\\n")
PY
"""
FAKE_GH_BEHAVIOUR = """if [[ "$*" == *"/actions/runs?"* ]]; then
  gh_failure="${NETSUKE_FAKE_GH_WORKFLOW_FAILURE:-}"
  gh_delay_seconds="${NETSUKE_FAKE_GH_WORKFLOW_DELAY_SECONDS:-}"
  gh_ignores_term="${NETSUKE_FAKE_GH_WORKFLOW_IGNORE_TERM:-}"
else
  gh_failure="${NETSUKE_FAKE_GH_FAILURE:-}"
  gh_delay_seconds="${NETSUKE_FAKE_GH_DELAY_SECONDS:-}"
  gh_ignores_term="${NETSUKE_FAKE_GH_IGNORE_TERM:-}"
fi
if [[ "$gh_failure" == "true" ]]; then exit 1; fi
if [[ "$gh_ignores_term" == "true" ]]; then trap '' TERM; while :; do sleep 1; done; fi
if [[ -n "$gh_delay_seconds" ]]; then sleep "$gh_delay_seconds"; fi
if [[ "$*" == *"/commits/"* ]]; then
  printf '%s\\n' "${NETSUKE_FAKE_RESOLVED_REVISION:-$GITHUB_SHA}"
else
  printf '%s\\n' "${NETSUKE_FAKE_WORKFLOW_RUN_ID-1001}"
fi
"""
FAKE_GIT_BEHAVIOUR = """if [[ "${NETSUKE_FAKE_GIT_FAILURE:-}" == "true" ]]; then
  exit 1
fi
"""


def write_fake_commands(directory: Path) -> Path:
    """Write fake GitHub and Git adapters that record each native call.

    Parameters
    ----------
    directory
        Directory that receives the executable fakes and call log.

    Returns
    -------
    Path
        JSON Lines call-log path shared by both fake adapters.

    Notes
    -----
    Contract invariant: fakes preserve native arguments without altering the
    production adapter contract.
    """
    call_log = directory / "command-calls.jsonl"
    call_log.touch()
    for name, behaviour in (("gh", FAKE_GH_BEHAVIOUR), ("git", FAKE_GIT_BEHAVIOUR)):
        adapter = directory / name
        adapter.write_text(
            FAKE_ADAPTER_PREAMBLE.replace("ADAPTER_NAME", name) + behaviour,
            encoding="utf-8",
        )
        adapter.chmod(0o755)
    return call_log
