"""Bind each registry decision to the steps declared by the composite action."""

import pytest
from action_run_sources import action_sources, action_steps
from release_publish_crates_io_contracts import (
    ACTION_PATH,
    ACTION_STEP_ORDER,
    PUBLISH_ONLY,
)

_BASE_SEQUENCE = ("candidate", "probe", "decision")
_PUBLISH_SEQUENCE = (
    *_BASE_SEQUENCE,
    "auth",
    "metadata",
    "upload",
    "download",
    "verify",
)
_SKIP_SEQUENCE = (*_BASE_SEQUENCE, "download", "verify")
_FAIL_SEQUENCE = _BASE_SEQUENCE


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
    if after_failed_decision:
        return False
    if guard is None:
        return True
    assert guard == PUBLISH_ONLY, f"unexpected action guard: {guard!r}"
    return decision == "publish"


@pytest.mark.parametrize(
    ("state", "decision", "expected_sequence"),
    [
        pytest.param("absent", "publish", _PUBLISH_SEQUENCE, id="absent-publishes"),
        pytest.param("identical", "skip", _SKIP_SEQUENCE, id="identical-verifies"),
        pytest.param("conflict", "fail", _FAIL_SEQUENCE, id="conflict-stops"),
        pytest.param("yanked", "fail", _FAIL_SEQUENCE, id="yanked-stops"),
        pytest.param(
            "indeterminate",
            "fail",
            _FAIL_SEQUENCE,
            id="malformed-response-stops",
        ),
        pytest.param(
            "indeterminate",
            "fail",
            _FAIL_SEQUENCE,
            id="unavailable-response-stops",
        ),
    ],
)
def test_registry_state_resolves_the_declared_action_sequence(
    state: str, decision: str, expected_sequence: tuple[str, ...]
) -> None:
    """Evaluate the manifest guards for every registry state and outcome."""
    actual_sequence = _steps_for_decision(decision)

    assert actual_sequence == expected_sequence, (
        f"the {state} state must resolve to the {decision} action sequence"
    )
