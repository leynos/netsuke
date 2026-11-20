"""Break the gate's derived enforcement mode one spelling at a time.

The admission gate derives enforcement from the release mode rather than
configuring it beside the mode, and it recognises only the exact lowercase
``true`` and ``false`` the runner renders for a boolean. A value that is
uppercase, padded, or not an expression at all reaches the gate as different
text, so a publishing run observes instead of enforcing: it succeeds while
admitting nothing. Each mutation states one of those spellings, so the
contract's exact-match rule and its wrapper rule are shown to fire rather than
merely hold.

The mutations are exported as :data:`ENFORCEMENT_MUTATIONS` and merged into the
publication gate's mutation set in :mod:`release_publication_gate_mutations`.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_admission_gate_contract import (
    ADMISSION_JOB,
    ADMISSION_STEP_ID,
    ENFORCE_VARIABLE,
)
from release_mutation_support import MutationError, MutationIssue
from workflow_loading import job_steps, require_mapping

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def _environment(release: dict[str, object]) -> dict[str, object]:
    """Return the gate step's environment, refusing a step that has moved.

    Returns
    -------
    dict[str, object]
        The mutable environment mapping the case rewrites.

    Raises
    ------
    MutationError
        If the gate step or its environment is no longer where the case
        expects, so the mutation cannot state the regression it names.
    """
    step = next(
        (
            item
            for item in job_steps(release, ADMISSION_JOB)
            if item.get("id") == ADMISSION_STEP_ID
        ),
        None,
    )
    if step is None:
        raise MutationError(MutationIssue.PATTERN_MISMATCH, ADMISSION_STEP_ID)
    return require_mapping(step.get("env"), f"{ADMISSION_JOB} step environment")


def _set_enforcement(release: dict[str, object], value: object) -> None:
    """Configure enforcement with one spelling of the mode."""
    _environment(release)[ENFORCE_VARIABLE] = value


def _bare_expression(release: dict[str, object]) -> None:
    """Drop the expression wrapper, leaving text the gate receives literally.

    A step environment is interpolated only inside ``${{ }}``; without the
    delimiters the gate compares the expression's own source against the mode
    and observes.

    Raises
    ------
    MutationError
        If the declared value is not a wrapped expression, so there is no
        wrapper to remove and the case cannot be stated.
    """
    environment = _environment(release)
    text = environment.get(ENFORCE_VARIABLE)
    if not isinstance(text, str):
        raise MutationError(MutationIssue.PATTERN_MISMATCH, ENFORCE_VARIABLE)
    stripped = text.strip()
    if not (stripped.startswith("${{") and stripped.endswith("}}")):
        raise MutationError(MutationIssue.PATTERN_MISMATCH, ENFORCE_VARIABLE)
    environment[ENFORCE_VARIABLE] = stripped[3:-2].strip()


def _uppercase(release: dict[str, object]) -> None:
    """Configure the mode in the uppercase spelling the gate does not accept."""
    _set_enforcement(release, "${{ 'TRUE' }}")


def _padded(release: dict[str, object]) -> None:
    """Configure the mode with surrounding whitespace the gate does not trim."""
    _set_enforcement(release, "${{ ' true ' }}")


ENFORCEMENT_MUTATIONS: dict[str, cabc.Callable[[dict[str, object]], None]] = {
    "enforcement-bare-expression": _bare_expression,
    "enforcement-uppercase": _uppercase,
    "enforcement-padded": _padded,
}

__all__ = ("ENFORCEMENT_MUTATIONS",)
