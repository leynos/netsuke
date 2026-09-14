"""What a job running the CodeScene changed-line gate has to look like.

`upload-codescene-coverage` documents `mode: check` as diffing against a
merge base and requiring a `fetch-depth: 0` checkout. These predicates
say when a job satisfies that, reading job mappings rather than files,
so the contract in ``codescene_check_depth_test`` can hold the
repository's own workflows to it and
``codescene_check_depth_properties_test`` can drive shapes the
repository does not have.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

#: The shared action whose `check` mode runs the CodeScene CLI.
CODESCENE_COVERAGE_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/upload-codescene-coverage"
)

#: The action that provides the checkout the CLI reads its history from.
CHECKOUT_ACTION: typ.Final[str] = "actions/checkout"

#: `fetch-depth: 0` is the whole history rather than an empty one, and is
#: what the shared action documents `mode: check` as requiring.
FULL_HISTORY_DEPTH: typ.Final[int] = 0

#: What `actions/checkout` fetches when a step names no `fetch-depth`.
DEFAULT_FETCH_DEPTH: typ.Final[int] = 1


def action_of(step: dict[str, object]) -> str:
    """Return a step's action reference without its version, or the empty string."""
    # Split on the version separator rather than matching a prefix: a
    # prefix match would accept `upload-codescene-coverage-legacy`, and a
    # substring match would accept an action merely mentioning the name.
    uses = step.get("uses")
    return uses.split("@", 1)[0] if isinstance(uses, str) else ""


def runs_the_codescene_check(step: dict[str, object]) -> bool:
    """Return whether a step runs the CodeScene CLI's changed-line gate."""
    # `mode: check` is the mode that reads git history. An upload-only
    # step submits the report and diffs nothing, so it needs no history
    # and is not held to this depth.
    if action_of(step) != CODESCENE_COVERAGE_ACTION:
        return False
    with_block = step.get("with")
    return isinstance(with_block, dict) and with_block.get("mode") == "check"


def declared_depth(step: dict[str, object]) -> int:
    """Return a checkout step's ``fetch-depth``, or the action's default."""
    # A workflow may write the depth as a YAML integer or as a quoted
    # string; both reach the action as the same value, so both are read
    # here. Anything else is not a depth and reads as the default, which
    # fails the assertion rather than passing unexamined.
    with_block = step.get("with")
    if not isinstance(with_block, dict) or "fetch-depth" not in with_block:
        return DEFAULT_FETCH_DEPTH
    match with_block["fetch-depth"]:
        case bool():
            # `True` is an `int` in Python and is not a depth.
            return DEFAULT_FETCH_DEPTH
        case int() as depth:
            return depth
        case str() as text if text.strip().lstrip("-").isdigit():
            return int(text.strip())
        case _:
            return DEFAULT_FETCH_DEPTH


def fetches_whole_history(step: dict[str, object]) -> bool:
    """Return whether a step is a checkout fetching the whole history."""
    return (
        action_of(step) == CHECKOUT_ACTION
        and declared_depth(step) == FULL_HISTORY_DEPTH
    )


def _first_gate(steps: list[dict[str, object]]) -> int | None:
    """Return the index of the first gate step, or None."""
    return next(
        (index for index, step in enumerate(steps) if runs_the_codescene_check(step)),
        None,
    )


def _first_whole_history_checkout(steps: list[dict[str, object]]) -> int | None:
    """Return the index of the first full-history checkout, or None."""
    return next(
        (index for index, step in enumerate(steps) if fetches_whole_history(step)),
        None,
    )


def gate_is_prepared(steps: list[dict[str, object]]) -> bool:
    """Return whether a full-history checkout precedes the first gate step."""
    # Both indices come from the same step list, so comparing them is
    # what states the ordering. A checkout after the gate leaves the same
    # empty history when the CLI runs as no checkout at all.
    gate = _first_gate(steps)
    checkout = _first_whole_history_checkout(steps)
    return gate is not None and checkout is not None and checkout < gate


def runs_the_gate(steps: list[dict[str, object]]) -> bool:
    """Return whether any step in a job runs the changed-line gate."""
    return _first_gate(steps) is not None
