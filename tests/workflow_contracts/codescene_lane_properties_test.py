"""Properties for the report-delivery contract, over generated edits.

The cases in ``codescene_upload_contract_test`` vary one field at a time by
hand, which holds the contract to the faults somebody thought to write down.
These state the same contract over a *space*: ``codescene_lane_edits`` draws one
bounded edit to a valid lane, ``codescene_lane_edit_model`` records what the edit
changed and whether the contract must report it, and the property below compares
that recorded verdict with what ``upload_contract_offenders`` actually returns.

The oracle is the contract itself, read over a lane the model edited. That is
what makes the comparison worth making: the model's verdict is an independent
account of the edit, decided before the lane existed, so a disagreement is
between the contract and the model rather than between two readings of the same
text.

Run via ``make test-workflow-contracts``.
"""

import functools
import typing as typ

from codescene_lane_edit_model import (
    FAMILIES,
    REQUIRED_FIELDS,
    REQUIRED_INPUTS,
    Mutation,
)
from codescene_lane_edits import mutations
from codescene_upload_invariants import REPORT_STEP_NAMES, upload_contract_offenders
from codescene_upload_lane_data import clean_steps, inputs_of, step_of
from hypothesis import find, given, settings

#: One mutation, drawn to order rather than by sampling: the seeded generator
#: replays identically, so seeking a family by name lands on the same example
#: every run, and no example is discarded for taking too long. The house
#: settings, as a value, for the `find` calls below; the property decorates
#: itself with the same two parameters.
PROPERTY_SETTINGS: typ.Final[settings] = settings(derandomize=True, deadline=None)


def _is_of_family(family: str, mutation: Mutation) -> bool:
    """Return whether ``mutation`` belongs to ``family``.

    Returns
    -------
    bool
        True when the mutation's recorded family is the one sought.
    """
    return mutation.family == family


def _lane_steps() -> list[dict[str, object]]:
    """Return a fresh copy of the clean lane's steps.

    Returns
    -------
    list[dict[str, object]]
        The steps, owned by the caller.
    """
    return clean_steps()


def test_the_clean_lane_carries_every_condition_the_model_removes() -> None:
    """Hold the fixture to the fields and inputs the model assumes it has.

    The model states the required fields and the required order itself rather
    than reading them back out of the lane, and every removal it proposes is
    against something the contract asks for. That is only sound while the
    fixture actually carries them: a removal of a field the lane never had would
    change nothing, and a mutation that changes nothing would let the contract
    pass the removal property without reading anything.
    """
    steps = _lane_steps()
    for step_name, field_names in REQUIRED_FIELDS.items():
        step = step_of(steps, step_name)
        missing = [field for field in field_names if field not in step]
        assert not missing, (
            f"the clean lane must carry {missing!r} on {step_name!r}; "
            f"the whole removal family is stated over them"
        )
    for step_name, input_names in REQUIRED_INPUTS.items():
        step = step_of(steps, step_name)
        if not input_names:
            # The validating step is a `run` block and declares no inputs; the
            # table says so rather than leaving the step out, so the two
            # directions — every step listed, every listed input present — are
            # both stated here.
            assert "with" not in step, (
                f"the clean lane must declare no inputs on {step_name!r}; "
                f"{input_names!r} is the contract's own statement "
                f"that it has none"
            )
            continue
        submitted = inputs_of(step)
        missing = [name for name in input_names if name not in submitted]
        assert not missing, (
            f"the clean lane must carry {missing!r} on {step_name!r}; "
            f"the whole removal family is stated over them"
        )
    assert not upload_contract_offenders(steps), (
        "the lane every property starts from must satisfy the contract"
    )


def test_the_order_the_model_states_is_the_order_the_lane_has() -> None:
    """Hold the model's required order to the lane it mutates.

    A reordering is reportable because the model knows where the contract wants
    the step; if the fixture were declared in another order, every one of them
    would be a fault claimed for an edit that repaired the lane instead.
    """
    steps = _lane_steps()
    positions = {name: steps.index(step_of(steps, name)) for name in REPORT_STEP_NAMES}
    assert list(positions.values()) == sorted(positions.values()), (
        f"the clean lane must declare {REPORT_STEP_NAMES!r} in order; "
        f"they sit at {positions!r}"
    )


def test_every_family_and_both_verdicts_are_reachable() -> None:
    """Hold the generator to producing faults and controls alike.

    A property comparing a recorded verdict with the contract's own is only
    worth stating if both verdicts are drawn, and if each family of fault is
    reached at all. ``find`` asks for one mutation of each family, which is what
    "reached" means here; the control is asked for separately, since a generator
    that had quietly stopped producing one would leave the accept direction
    unexercised while every other property still passed.
    """
    for family in FAMILIES:
        # Bound rather than closed over: a lambda reading the loop variable
        # would ask for whichever family the loop had reached by the time the
        # generator called it back, which is none of the answer this test wants.
        of_family = functools.partial(_is_of_family, family)
        mutation = find(mutations(), of_family, settings=PROPERTY_SETTINGS)
        assert mutation.family == family
        if mutation.reportable:
            assert mutation.named, (
                f"a reportable edit of family {family!r} must say which names "
                f"an offender has to use; {mutation.description} names none"
            )
        else:
            assert not mutation.named, (
                f"an edit the contract must accept names nothing it changed; "
                f"{mutation.description} names {mutation.named!r}"
            )

    control = find(
        mutations(), lambda drawn: not drawn.reportable, settings=PROPERTY_SETTINGS
    )
    assert control.family in FAMILIES
    fault = find(
        mutations(), lambda drawn: drawn.reportable, settings=PROPERTY_SETTINGS
    )
    assert fault.reportable
    assert fault.named, "a reportable edit must say what an offender has to name"


@settings(max_examples=300, derandomize=True, deadline=None)
@given(mutation=mutations())
def test_the_contract_reports_exactly_the_edits_the_model_calls_faults(
    mutation: Mutation,
) -> None:
    """Compare the contract's verdict on an edit with the model's.

    The model builds the edit and records, at that moment, whether a valid lane
    becomes one the contract must refuse. The lane is then handed to
    ``upload_contract_offenders``, which reads it and decides for itself — and
    the two must agree, in both directions. A contract that reported a lane the
    model only inserted an unrelated step into would be refusing a correct
    workflow; one that accepted a reversed gate would be passing a lane that
    never uploads a report.

    When the model says the lane must be reported, the property goes further
    than "something was reported": an offender has to *name* what the edit
    changed, so a rule that fired for an unrelated reason cannot stand in for
    the rule the family is about.
    """
    steps = _lane_steps()
    mutation.apply(steps)
    offenders = upload_contract_offenders(steps)

    assert bool(offenders) == mutation.reportable, (
        f"{mutation.description}: the contract must "
        f"{'report' if mutation.reportable else 'accept'} the lane; got {offenders!r}"
    )
    if not mutation.named:
        return
    named = [
        offender
        for offender in offenders
        if any(token in offender for token in mutation.named)
    ]
    assert named, (
        f"{mutation.description}: an offender must name one of "
        f"{mutation.named!r}; got {offenders!r}"
    )
