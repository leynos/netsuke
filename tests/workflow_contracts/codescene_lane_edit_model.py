"""One bounded change to a valid lane, and the contract's verdict on it.

A contract test has to be shown a lane that satisfies it before it can vary one
field and assert the offender that results. The cases that do this one at a time
by hand live beside the lane they drive; this model states the same question
over a *space*: an edit to a valid lane is described here by what it changed and
by whether the contract must report the result.

Recording the verdict at construction is what makes the property worth stating.
The model decides what it changed and what that change means; the contract reads
the lane and decides for itself. A disagreement is therefore between the
contract and an independent account of the edit, rather than between two
readings of the same text.

Every mutation names the family of required condition it breaks. The families
are the ones the lane's contract is stated over: a carrier *removed*, a step
*duplicated*, a step *reordered*, an input *misbound*, a step *ungated*, the
gate *reversed*, and the gate *weakened*. One family is of a different kind — an
unrelated step *inserted* into an otherwise valid lane — because a rule that
reported everything would be as useless as one that reported nothing, and only a
control separates the two.

The model records which names the edit touched, so the property can ask more of
a reportable edit than that *something* was reported: an offender has to name
one of them. Which clause answers is a fact about the contract rather than about
the edit, so the recorded set holds every name the change is visible under — the
step it was made to, the field or input it was made in, and the credential when
the fault is in how the upload is wired.

This module holds the vocabulary and says nothing about how an edit is drawn or
built: ``codescene_lane_mutations`` holds the constructors and
``codescene_lane_edits`` the strategies. Nothing here decides anything about a
lane by reading it. The required fields and the required order are this module's
own statement of what the contract asks for, and
``codescene_lane_properties_test`` holds the clean fixture to carrying every one
of them: a mutation proposed against a field the fixture does not have would
change nothing, and a mutation that changes nothing proves nothing about the
contract.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import dataclasses as dc
import typing as typ

from codescene_credential_invariants import (
    CREDENTIAL_ENVIRONMENT_KEY,
    CREDENTIAL_INPUT,
)
from codescene_report_validation_invariants import REPORT_VALIDATION_STEP
from codescene_upload_invariants import (
    CODESCENE_UPLOAD_STEP,
    COVERAGE_FORMAT_INPUT,
    COVERAGE_STEP,
    OUTPUT_PATH_INPUT,
    UPLOAD_PATH_INPUT,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The fields each guarded step must carry for the lane to satisfy the
#: contract. The clean fixture carries every one — asserted in the test module,
#: not assumed here — so removing one is a real change to a lane that had it.
REQUIRED_FIELDS: typ.Final[dict[str, tuple[str, ...]]] = {
    COVERAGE_STEP: ("uses",),
    REPORT_VALIDATION_STEP: ("run",),
    CODESCENE_UPLOAD_STEP: ("uses", "if", "env"),
}

#: The `with` inputs each guarded step must carry, for the same reason.
REQUIRED_INPUTS: typ.Final[dict[str, tuple[str, ...]]] = {
    COVERAGE_STEP: (OUTPUT_PATH_INPUT, COVERAGE_FORMAT_INPUT),
    REPORT_VALIDATION_STEP: (),
    CODESCENE_UPLOAD_STEP: (
        UPLOAD_PATH_INPUT,
        COVERAGE_FORMAT_INPUT,
        CREDENTIAL_INPUT,
    ),
}

#: The ways a required condition can be broken. Named rather than positional so
#: a failure says which kind of rule the contract failed to apply, and so the
#: test module can assert that the generator reaches every one of them.
REMOVAL: typ.Final[str] = "removal"
DUPLICATION: typ.Final[str] = "duplication"
REORDERING: typ.Final[str] = "reordering"
MISBINDING: typ.Final[str] = "misbinding"
UNGATING: typ.Final[str] = "ungating"
REVERSAL: typ.Final[str] = "reversal"
WEAKENING: typ.Final[str] = "weakening"
INSERTION: typ.Final[str] = "insertion"

#: Every family, the control included.
FAMILIES: typ.Final[tuple[str, ...]] = (
    REMOVAL,
    DUPLICATION,
    REORDERING,
    MISBINDING,
    UNGATING,
    REVERSAL,
    WEAKENING,
    INSERTION,
)


@dc.dataclass(frozen=True)
class Mutation:
    """One bounded edit to a lane, and the contract's verdict on the result."""

    #: The family this mutation belongs to.
    family: str
    #: What the edit is, for a failure message.
    description: str
    #: The edit, applied to a lane in place.
    apply: cabc.Callable[[list[dict[str, object]]], None]
    #: Whether the contract must report the mutated lane.
    reportable: bool
    #: The names the edit is visible under, one of which an offender must use.
    #:
    #: A rule that reported *something* would satisfy a claim that the lane is
    #: reported, so the claim is sharpened: an offender has to name the change,
    #: not merely fire beside it. The set holds every name the change can be
    #: read under — the step it was made to, the field or input it was made in,
    #: and the credential when what was faulted is the upload's wiring — because
    #: which clause answers is a fact about the contract rather than about the
    #: edit, and a clause that describes the change at all uses one of them.
    #: Empty for an edit the contract must accept, which has nothing to name.
    named: tuple[str, ...] = ()


def visible_names(*names: str) -> tuple[str, ...]:
    """Return the names a change in the upload's wiring is visible under.

    A clause may describe such a change by the step it was made to, by the field
    it was made in, or by the credential the step's wiring is about — the gate,
    the environment entry, and the token handed to the action are three readings
    of one arrangement. So the credential is added whenever the change touches
    the upload, which is this module's own statement of what the step is for: a
    contract describing the change at all names one of these.

    Returns
    -------
    tuple[str, ...]
        The names, deduplicated and in the order given.
    """
    if CODESCENE_UPLOAD_STEP not in names:
        return names
    return (
        (*names, CREDENTIAL_ENVIRONMENT_KEY)
        if (CREDENTIAL_ENVIRONMENT_KEY not in names)
        else names
    )
