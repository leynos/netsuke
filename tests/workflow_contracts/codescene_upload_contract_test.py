"""Hold the trunk lane to the report-delivery contract CodeScene depends on.

`coverage-main.yml` is the only lane that hands a coverage report to CodeScene,
and it does so from the workspace: the report is written by the shared coverage
action and read by the shared upload action in the same job, with nothing
archiving, downloading, or restoring it in between. The contract is therefore
about ordering, about the two steps agreeing on a path and a format, and about
the upload being configured so it can actually run and actually verify
something.

A green run does not prove any of that. The upload asserts the file exists and
fails when it is missing, so a disagreement surfaces as a failure — but only
after the instrumented build, which is the most expensive thing the lane does,
and only on the trunk, where the result is already merged. Measured from the
outside the same fault looks like a CodeScene check that waits for a report and
then gives up hours later, which names neither the step nor the input at fault.

The predicates under test live in ``codescene_upload_invariants.py`` and in
``codescene_credential_invariants.py``, which owns the rules about the secret
the lane is handed, and the shared parsing helpers in ``workflow_loading.py``.
Following the sibling suites, the detectors are also driven against synthetic
workflow text: a detector that stopped matching would otherwise let the
repository assertion pass by finding nothing to object to.

Two subjects are split off from this one. What the *validating step's script*
must read as lives in ``codescene_validation_step_test.py``, and how the upload
is *configured* — the inputs it agrees on with the generator, the credential it
is handed, and the gate it is submitted under — lives in
``codescene_upload_configuration_test.py``. This module holds what is left: the
lane's structure, meaning which steps exist and where they sit, and that each
action is called once at an immutable pin.

Run via ``make test-workflow-contracts``.
"""

import copy

import pytest
from ci_coverage_wiring_invariants import GENERATE_COVERAGE_ACTION
from codescene_report_validation_invariants import REPORT_VALIDATION_STEP
from codescene_upload_invariants import (
    CODESCENE_UPLOAD_STEP,
    COVERAGE_STEP,
    upload_contract_offenders,
)
from codescene_upload_lane_data import (
    FIXTURE_PIN,
    clean_steps,
    step_of,
    trunk_steps,
)


def test_the_clean_lane_template_produces_no_offenders() -> None:
    """Keep the cases below from asserting against a broken baseline."""
    assert not upload_contract_offenders(clean_steps()), (
        "the template the negative cases vary must itself satisfy the contract"
    )


def test_the_trunk_lane_satisfies_the_report_delivery_contract() -> None:
    """Hold the repository's only CodeScene submission lane to the contract."""
    offenders = upload_contract_offenders(trunk_steps())
    assert not offenders, "; ".join(offenders)


@pytest.mark.parametrize(
    "step_name", [COVERAGE_STEP, REPORT_VALIDATION_STEP, CODESCENE_UPLOAD_STEP]
)
def test_the_detectors_report_a_lane_whose_steps_are_renamed(step_name: str) -> None:
    """Match the lane structurally, not by the names it happens to use.

    A renamed step would otherwise stop the ordering assertion from finding
    what it is about, and the whole contract would pass over an empty set.
    """
    steps = clean_steps()
    step_of(steps, step_name)["name"] = f"{step_name} (renamed)"
    assert upload_contract_offenders(steps), (
        f"renaming {step_name!r} must be reported as a missing step"
    )


@pytest.mark.parametrize(
    "step_name", [COVERAGE_STEP, REPORT_VALIDATION_STEP, CODESCENE_UPLOAD_STEP]
)
def test_the_detectors_report_a_lane_that_declares_a_step_twice(
    step_name: str,
) -> None:
    """Fail a lane holding two steps under one name, not just the first.

    GitHub keys nothing on a step's name, so the second step of a repeated name
    runs whether or not a contract looked at it. A contract that inspected the
    first match alone would report the lane clean while an unpinned or ungated
    duplicate executed beside the step it certified — which is what a
    copy-pasted step produces, since it keeps the original's name.
    """
    steps = clean_steps()
    duplicate = copy.deepcopy(step_of(steps, step_name))
    duplicate["uses"] = GENERATE_COVERAGE_ACTION
    steps.append(duplicate)
    offenders = upload_contract_offenders(steps)
    assert offenders, (
        f"a second step named {step_name!r} must be reported: the contract "
        f"examined only the step it found first"
    )


@pytest.mark.parametrize("duplicate_name", [" (legacy)", " (copy)"])
def test_the_detectors_report_a_second_caller_of_the_upload_action(
    duplicate_name: str,
) -> None:
    """Fail a lane submitting to CodeScene through a step the contract never reads.

    The steps the contract examines are found by name, so a copy of the upload
    under a different name would run beside the certified step with its inputs,
    its gate and its pin all unchecked. The sibling case above repeats a name,
    which is a different fault: there the lookup has two answers, here it has
    one and the second submission is invisible to it.
    """
    steps = clean_steps()
    duplicate = copy.deepcopy(step_of(steps, CODESCENE_UPLOAD_STEP))
    duplicate["name"] = f"{CODESCENE_UPLOAD_STEP}{duplicate_name}"
    steps.append(duplicate)
    assert upload_contract_offenders(steps), (
        f"a second step invoking the upload action as {duplicate['name']!r} must "
        f"be reported: the contract reads only the step it found by name"
    )


@pytest.mark.parametrize("step_name", [COVERAGE_STEP, CODESCENE_UPLOAD_STEP])
def test_the_detectors_accept_an_unrelated_caller_of_another_action(
    step_name: str,
) -> None:
    """Accept a second step naming the other action the contract reads.

    The rule is stated over the action the *upload* invokes, so a second caller
    of the generator the contract reads through its own deposited step must
    still be accepted: a check that counted callers of every external action
    rather than matching one identity would refuse a correct workflow, which is
    the failure mode that gets a contract deleted rather than fixed.
    """
    steps = clean_steps()
    unrelated: dict[str, object] = {
        "name": "Unrelated step",
        "uses": f"{GENERATE_COVERAGE_ACTION}@{FIXTURE_PIN}",
    }
    steps.insert(steps.index(step_of(steps, step_name)), unrelated)
    assert not upload_contract_offenders(steps), (
        f"an unrelated step invoking {GENERATE_COVERAGE_ACTION} must be accepted"
    )


@pytest.mark.parametrize(
    "step_name", [COVERAGE_STEP, REPORT_VALIDATION_STEP, CODESCENE_UPLOAD_STEP]
)
def test_the_detectors_report_a_reordered_lane(step_name: str) -> None:
    """Fail beside the generation, or after the upload that reads it.

    Two distinct faults, both of which leave the report sent unchecked: a step
    placed before the report exists has nothing to read, and a check placed
    after the upload is a check the upload never waited for.
    """
    steps = clean_steps()
    moved = steps.pop(steps.index(step_of(steps, step_name)))
    match step_name:
        case _ if step_name == REPORT_VALIDATION_STEP:
            # Move it to the front, before the report is written.
            steps.insert(0, moved)
        case _ if step_name == CODESCENE_UPLOAD_STEP:
            # Move the upload ahead of the check that is meant to gate it.
            steps.insert(steps.index(step_of(steps, REPORT_VALIDATION_STEP)), moved)
        case _:
            # Move the generation after the step that reads its output.
            steps.append(moved)
    assert upload_contract_offenders(steps), (
        f"misordering {step_name!r} must be reported"
    )
