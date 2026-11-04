"""Bind each registry decision to the steps declared by the composite action."""

import dataclasses as dc
import json
import os
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - test a fixed local shell helper.
import typing as typ
from pathlib import Path

import pytest
from action_run_sources import action_sources, action_steps
from publish_crates_io_cli import (
    EXIT_ACTION_REQUIRED,
    EXIT_OK,
    EXIT_RESPONSE_ERROR,
)
from publish_crates_io_cli import (
    main as classify_registry,
)
from release_publish_crates_io_contracts import (
    ACTION_PATH,
    ACTION_STEP_ORDER,
    PUBLISH_ONLY,
)
from release_publish_crates_io_timing_contracts import TIMING_HELPER_PATH

if typ.TYPE_CHECKING:
    import pathlib

VERSION = "1.2.3"
SHA256 = "ab" * 32
_BASE_SEQUENCE = ("candidate", "probe", "decision")
_PUBLISH_SEQUENCE = (
    *_BASE_SEQUENCE,
    "auth_start",
    "auth",
    "auth_end",
    "metadata",
    "upload",
    "download",
    "verify",
)
_SKIP_SEQUENCE = (*_BASE_SEQUENCE, "auth_end", "download", "verify")
_FAIL_SEQUENCE = (*_BASE_SEQUENCE, "auth_end")


@dc.dataclass(frozen=True, slots=True)
class RegistryScenario:
    """Pair one registry response with its expected action and sequence."""

    status: int
    body: bytes
    state: str
    action: str
    exit_code: int
    sequence: tuple[str, ...]


def _registry_response(*, checksum: str = SHA256, yanked: bool = False) -> bytes:
    """Encode a crates.io version observation for the intended release."""
    return json.dumps({
        "version": {"num": VERSION, "checksum": checksum, "yanked": yanked}
    }).encode("utf-8")


def _classify_registry_response(
    directory: pathlib.Path, scenario: RegistryScenario
) -> tuple[int, dict[str, object]]:
    """Run the policy CLI over one saved registry observation."""
    body_path = directory / "response.json"
    decision_path = directory / "decision.json"
    _ = body_path.write_bytes(scenario.body)
    result = classify_registry([
        "classify",
        "--status",
        str(scenario.status),
        "--body",
        str(body_path),
        "--version",
        VERSION,
        "--sha256",
        SHA256,
        "--output",
        str(decision_path),
    ])
    decision = json.loads(decision_path.read_text(encoding="utf-8"))
    return result, decision


def _steps_for_decision(decision: str) -> tuple[str, ...]:
    """Resolve the actual composite action's step guards for one decision."""
    action = action_sources()[ACTION_PATH]
    steps = action_steps(ACTION_PATH, action)
    identifiers = tuple(str(step.get("id", "")) for step in steps)
    assert identifiers == ACTION_STEP_ORDER, (
        "the action must declare its full step order"
    )
    decision_position = identifiers.index("decision")

    selected: list[str] = []
    for position, step in enumerate(steps):
        identifier = str(step["id"])
        guard = step.get("if")
        after_failed_decision = decision == "fail" and position > decision_position
        if _step_runs_for_decision(
            guard, decision, after_failed_decision=after_failed_decision
        ):
            selected.append(identifier)
    return tuple(selected)


def _step_runs_for_decision(
    guard: object, decision: str, *, after_failed_decision: bool
) -> bool:
    """Resolve guards and implicit success after a failed decision."""
    if guard == "always()":
        return True
    if after_failed_decision:
        return False
    if guard is None:
        return True
    assert guard == PUBLISH_ONLY, f"unexpected action guard: {guard!r}"
    return decision == "publish"


@pytest.mark.parametrize(
    "scenario",
    [
        pytest.param(
            RegistryScenario(404, b"", "absent", "publish", EXIT_OK, _PUBLISH_SEQUENCE),
            id="absent-publishes",
        ),
        pytest.param(
            RegistryScenario(
                200, _registry_response(), "identical", "skip", EXIT_OK, _SKIP_SEQUENCE
            ),
            id="identical-verifies",
        ),
        pytest.param(
            RegistryScenario(
                200,
                _registry_response(checksum="cd" * 32),
                "conflict",
                "fail",
                EXIT_ACTION_REQUIRED,
                _FAIL_SEQUENCE,
            ),
            id="conflict-stops",
        ),
        pytest.param(
            RegistryScenario(
                200,
                _registry_response(yanked=True),
                "yanked",
                "fail",
                EXIT_ACTION_REQUIRED,
                _FAIL_SEQUENCE,
            ),
            id="yanked-stops",
        ),
        pytest.param(
            RegistryScenario(
                200,
                b"[]",
                "indeterminate",
                "fail",
                EXIT_RESPONSE_ERROR,
                _FAIL_SEQUENCE,
            ),
            id="malformed-response-stops",
        ),
        pytest.param(
            RegistryScenario(
                503,
                b"busy",
                "indeterminate",
                "fail",
                EXIT_ACTION_REQUIRED,
                _FAIL_SEQUENCE,
            ),
            id="unavailable-response-stops",
        ),
    ],
)
def test_registry_state_resolves_the_declared_action_sequence(
    tmp_path: pathlib.Path, scenario: RegistryScenario
) -> None:
    """Resolve each workflow sequence from the policy CLI's decision record."""
    result, decision = _classify_registry_response(tmp_path, scenario)

    assert result == scenario.exit_code, "classify each registry observation"
    assert decision["state"] == scenario.state, "record the observed registry state"
    assert decision["action"] == scenario.action, "record its permitted action"
    actual_sequence = _steps_for_decision(str(decision["action"]))

    assert actual_sequence == scenario.sequence, (
        f"the {scenario.state} state must resolve to the "
        f"{scenario.action} action sequence"
    )


def test_publisher_timing_trap_preserves_step_failure(
    tmp_path: pathlib.Path,
) -> None:
    """Write both timing outputs while preserving a failed step's status."""
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    fake_date = fake_bin / "date"
    _ = fake_date.write_text(
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
        '[[ "$*" == "+%s%N" ]]\n'
        'if [[ -e "$DATE_CALLS" ]]; then\n'
        "  printf '2\\n' >\"$DATE_CALLS\"\n"
        "  printf '21123000000\\n'\n"
        "else\n"
        "  printf '1\\n' >\"$DATE_CALLS\"\n"
        "  printf '1000000000\\n'\n"
        "fi\n",
        encoding="utf-8",
    )
    fake_date.chmod(0o755)
    output_path = tmp_path / "github-output"
    action_directory = (
        Path(__file__).resolve().parents[2] / Path(TIMING_HELPER_PATH).parent
    )
    environment = os.environ.copy()
    environment.update({
        "DATE_CALLS": str(tmp_path / "date-calls"),
        "GITHUB_ACTION_PATH": str(action_directory),
        "GITHUB_OUTPUT": str(output_path),
        "PATH": os.pathsep.join((str(fake_bin), environment.get("PATH", ""))),
    })
    bash_path = shutil.which("bash")
    assert bash_path is not None, "the timing helper test requires bash"
    helper_command = "\n".join((
        'source "$GITHUB_ACTION_PATH/timing.sh"',
        "publisher_timing_start_step",
        "exit 23",
        "",
    ))
    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed helper script; shell is disabled.
        [
            bash_path,
            "--noprofile",
            "--norc",
            "-e",
            "-u",
            "-o",
            "pipefail",
            "-c",
            helper_command,
        ],
        capture_output=True,
        check=False,
        env=environment,
        text=True,
    )

    assert result.returncode == 23, "the EXIT trap must preserve a failed step"
    assert output_path.read_text(encoding="utf-8") == (
        "started_ns=1000000000\nended_ns=21123000000\n"
    ), "record the shared timing fields from the helper"
