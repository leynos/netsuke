"""Finding a named step in a parsed lane, and checking what it invokes.

These are the two questions every lane contract asks before it can ask
anything else: which step is the one under examination, and does it call
the action the repository depends on at a pin that cannot move under it.
Neither answer is a fact about a particular lane, so they live here rather
than beside one lane's predicates.

The action check delegates to ``action_references``, which owns both halves
of the rule — the identity the step must name and the shape its pin must
have. What this module adds is the reporting: a lane contract returns a
list of offenders rather than aborting at the first fault, so a bad
reference has to come back as a string the caller can collect.

Separated from ``codescene_upload_invariants`` so neither module outgrows
the 400-line limit the Python lint gate enforces.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from action_references import require_external_action_sha

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def step_named(
    steps: cabc.Sequence[dict[str, object]], name: str
) -> dict[str, object] | None:
    """Return the single step called ``name``, or None when there is none.

    Parameters
    ----------
    steps
        Parsed workflow steps in declaration order.
    name
        The step name to find.

    Returns
    -------
    dict[str, object] or None
        The matching step, or None when no step carries that name. A duplicated
        name returns the first match; a caller's ordering assertion is what
        makes a second one matter, and a caller that needs uniqueness asserts
        the count itself.
    """
    return next((step for step in steps if step.get("name") == name), None)


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
