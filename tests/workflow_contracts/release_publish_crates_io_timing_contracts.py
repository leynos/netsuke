"""Keep publisher phase timing behind the action-local shell helper."""

from pathlib import Path

TIMING_HELPER_PATH = ".github/actions/publish-crates-io/timing.sh"
TIMING_HELPER_SOURCE = 'source "$GITHUB_ACTION_PATH/timing.sh"'


def check_action_timing_boundary(
    steps: list[dict[str, object]],
    positions: dict[str, int],
    violations: list[str],
) -> None:
    """Require publisher timing to use its scoped helper and stable outputs."""
    if -1 in positions.values():
        return
    _check_timed_action_steps(steps, positions, violations)
    _check_authentication_timing(steps, positions, violations)
    _check_timing_helper_file(violations)


def _check_timed_action_steps(
    steps: list[dict[str, object]], positions: dict[str, int], violations: list[str]
) -> None:
    """Require each ordinary action step to use the shared timing boundary."""
    timed_steps = (
        "candidate",
        "probe",
        "decision",
        "metadata",
        "upload",
        "download",
        "verify",
    )
    for name in timed_steps:
        run = str(steps[positions[name]].get("run", ""))
        if not _uses_timing_helper(run, "publisher_timing_start_step"):
            violations.append(
                f"publish-crates-io.{name}-timing: use the publisher timing helper"
            )
        if "date +%s%N" in run:
            violations.append(
                f"publish-crates-io.{name}-timing: do not sample runner time inline"
            )


def _uses_timing_helper(run: str, function_name: str) -> bool:
    """Return whether a shell block loads and calls one timing helper function."""
    return TIMING_HELPER_SOURCE in run and function_name in run


def _check_authentication_timing(
    steps: list[dict[str, object]], positions: dict[str, int], violations: list[str]
) -> None:
    """Keep authentication timing observable when the exchange fails."""
    auth_start = steps[positions["auth_start"]]
    auth_start_run = str(auth_start.get("run", ""))
    if not _uses_timing_helper(auth_start_run, "publisher_timing_start"):
        violations.append(
            "publish-crates-io.auth-start-timing: use the publisher timing helper"
        )
    if "date +%s%N" in auth_start_run:
        violations.append(
            "publish-crates-io.auth-start-timing: do not sample runner time inline"
        )

    auth_end = steps[positions["auth_end"]]
    auth_end_run = str(auth_end.get("run", ""))
    if auth_end.get("if") != "always()":
        violations.append(
            "publish-crates-io.auth-end-timing: retain unconditional failure timing"
        )
    if not _uses_timing_helper(auth_end_run, "publisher_timing_end"):
        violations.append(
            "publish-crates-io.auth-end-timing: finish only a started auth phase"
        )
    if 'if [[ -n "$AUTH_STARTED_NS" ]]' not in auth_end_run:
        violations.append(
            "publish-crates-io.auth-end-timing: finish only a started auth phase"
        )
    if "date +%s%N" in auth_end_run:
        violations.append(
            "publish-crates-io.auth-end-timing: do not sample runner time inline"
        )


def _check_timing_helper_file(violations: list[str]) -> None:
    """Require the helper to preserve timing outputs and the original exit code."""
    helper_path = Path(__file__).resolve().parents[2] / TIMING_HELPER_PATH
    try:
        helper_source = helper_path.read_text(encoding="utf-8")
    except OSError:
        violations.append("publish-crates-io.timing-helper: helper file is unavailable")
        return
    required_helper_fragments = (
        "publisher_timing_start()",
        "publisher_timing_end()",
        "publisher_timing_start_step()",
        "publisher_timing_finish_step()",
        "trap publisher_timing_finish_step EXIT",
        "local step_exit_status=$?",
    )
    if any(fragment not in helper_source for fragment in required_helper_fragments):
        violations.append(
            "publish-crates-io.timing-helper: preserve start, end, and exit trap"
        )
    if helper_source.count("date +%s%N") != 2:
        violations.append(
            "publish-crates-io.timing-helper: keep clock reads behind one boundary"
        )
