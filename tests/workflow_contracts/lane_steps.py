"""Finding a named step in a parsed lane, and checking what it invokes.

These are the two questions every lane contract asks before it can ask
anything else: which step is the one under examination, and does it call
the action the repository depends on at a pin that cannot move under it.
Neither answer is a fact about a particular lane, so they live here rather
than beside one lane's predicates.

The first question has a precondition the second does not: a name carried by
more than one step has no single answer, and a lane contract that read the
first match would certify one step while a second of the same name ran beside
it. GitHub does not require names to be unique, so
[`step_names_declared_twice`] answers that separately, as the fault it is,
rather than letting the lookup quietly narrow the question.

The action check delegates to ``action_references``, which owns both halves
of the rule — the identity the step must name and the shape its pin must
have. What this module adds is the reporting: a lane contract returns a
list of offenders rather than aborting at the first fault, so a bad
reference has to come back as a string the caller can collect.

Separated from ``codescene_upload_invariants`` so neither module outgrows
the 400-line limit the Python lint gate enforces.

Run via ``make test-workflow-contracts``.
"""

import collections
import typing as typ

from action_references import require_external_action_sha

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def step_named(
    steps: cabc.Sequence[dict[str, object]], name: str
) -> dict[str, object] | None:
    """Return the first step called ``name``, or None when there is none.

    This is the lookup, not the precondition. A name carried by more than one
    step has no single answer, so a caller about to make a claim of *the* step
    so named must first rule that out with [`step_names_declared_twice`] — a
    contract that inspected the first match alone would certify one step while
    a second of the same name ran beside it, and would report the lane clean.

    Returning the first match rather than raising is what keeps the two
    questions separable: a caller that only wants to know whether a name is
    declared is answered here and is not the one that has to distinguish the
    cases, so the uniqueness assertion sits with the callers that need it
    instead of aborting the scan of the ones that do not.

    Parameters
    ----------
    steps
        Parsed workflow steps in declaration order.
    name
        The step name to find.

    Returns
    -------
    dict[str, object] or None
        The first step carrying that name, or None when no step does.
    """
    return next((step for step in steps if step.get("name") == name), None)


def step_names_declared_twice(
    steps: cabc.Sequence[dict[str, object]],
) -> list[str]:
    """Return every step name that ``steps`` declares more than once.

    A lane is written by hand and read by these contracts, and the two readings
    diverge the moment a name repeats. GitHub keys nothing on a step's name, so
    a copy-pasted step keeps its original name and runs: two uploads, the
    second unpinned or ungated, executing in a lane whose every step this
    contract examined. The lookup can only ever return one of them, so a
    duplicated name is reported as the fault it is rather than narrowed away.

    Parameters
    ----------
    steps
        Parsed workflow steps in declaration order.

    Returns
    -------
    list[str]
        One entry per repeated name, in the order each repetition is first
        seen, empty when every name is unique.
    """
    counts = collections.Counter(
        name for name in (step.get("name") for step in steps) if isinstance(name, str)
    )
    return [name for name, count in counts.items() if count > 1]


def inputs_of(step: dict[str, object]) -> dict[str, object]:
    """Return a step's ``with`` block, or an empty mapping when it has none."""
    with_ = step.get("with")
    return with_ if isinstance(with_, dict) else {}


def action_reference_of(
    step: dict[str, object], description: str, expected_action: str
) -> str | None:
    """Return the fault in a step's action reference, or None when it is sound.

    The step must call ``expected_action`` and pin it to a full commit SHA.
    Which revision that is belongs to the dependency updater, so it is not
    asserted: the shared checker owns both halves of this rule, and this
    wrapper only turns its failure into an offender string so one bad
    reference joins the other findings in the caller's report rather than
    aborting the scan at the first one.

    Parameters
    ----------
    step
        The step whose ``uses`` is checked.
    description
        The step name, for the failure text.
    expected_action
        The action path the step must name.

    Returns
    -------
    str or None
        The fault, or None when the step names the action at a full pin.
    """
    try:
        require_external_action_sha(step.get("uses"), expected_action, description)
    except AssertionError as fault:
        return f"{description!r} must invoke {expected_action} at a full pin: {fault}"
    return None
