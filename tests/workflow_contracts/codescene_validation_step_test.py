"""What the report-validating step's script must read as.

The lane-structure tests in ``codescene_upload_contract_test`` ask whether the
step is present, unique, and in the right place. These ask the other half:
whether the script it runs actually reads the report the upload will send. Each
is driven by a table in ``codescene_validation_step_data``, so a case states
only what it varies — which file is copied, into what, and whether anything
created the directory first.

The cases come in two kinds. Most vary one field of a script that satisfies the
contract and assert the offender that results, which is what a detector that
stopped matching would fail to report. The rest are controls: a script that
looks like a rejected case but stages the report correctly, asserted to produce
no offender at all. Without them a detector scoped ever more finely would be
driven to reject the correct script too, and the table would report success
while the lane it guards could no longer be written.

Separated from ``codescene_upload_contract_test`` so neither module outgrows
the 400-line limit the Python lint gate enforces.

Run via ``make test-workflow-contracts``.
"""

import pytest
from codescene_report_validation_invariants import REPORT_VALIDATION_STEP
from codescene_upload_invariants import upload_contract_offenders
from codescene_upload_lane_data import clean_steps, step_of
from codescene_validation_step_data import (
    CHECKS_NOTHING_CASES,
    COMMAND_SCOPED_CASES,
    COPY_DIRECTION_CASES,
    CREATION_BINDING_CASES,
)


def _offenders_for(replacement: str) -> list[str]:
    """Return the offenders reported for a lane running ``replacement``.

    Every case is the clean lane with one script varied, so the arrangement is
    stated once here rather than restated by each test. The step is located by
    name, as the lane-structure tests do, and its script replaced wholesale.

    Parameters
    ----------
    replacement
        The ``run`` script to drive the detectors with.

    Returns
    -------
    list[str]
        Every offender the contract reports for the varied lane.
    """
    steps = clean_steps()
    step_of(steps, REPORT_VALIDATION_STEP)["run"] = replacement
    return upload_contract_offenders(steps)


def _matching(offenders: list[str], expected: str) -> list[str]:
    """Return the offenders carrying ``expected``.

    A script can be reported for more than one fault, so a case naming the one
    it is about must not be satisfied by an offender for some other — which is
    what a bare truthiness check on the list would certify.

    Parameters
    ----------
    offenders
        Every offender reported for the case's lane.
    expected
        The text the relevant offender must carry.

    Returns
    -------
    list[str]
        The offenders naming the expected fault, empty when none does.
    """
    return [offender for offender in offenders if expected in offender]


@pytest.mark.parametrize(("replacement", "expected"), CHECKS_NOTHING_CASES)
def test_the_detectors_report_a_validation_step_that_checks_nothing(
    replacement: str, expected: str
) -> None:
    """Fail a check that does not read the report the upload will send.

    The generation action reports success for an empty report, so a step that
    only asserts the file exists would pass on exactly the artefact CodeScene
    rejects — and it would do so in the lane that runs most expensively. The
    same is true of a step that reads the right validator over a directory it
    never put the report into: everything about the invocation looks correct,
    and none of it touches the artefact. So is a step that moves the report
    into that directory: the check reads the artefact, and leaves the
    workspace without the copy the upload has yet to make. So is a step that
    names a directory it never created: the validator is pointed at a path
    that either is not there or holds another run's leavings. And so is a step
    that fills no directory at all while reading one: the validator refuses
    the empty set on a report nothing was wrong with.
    """
    matching = _matching(_offenders_for(replacement), expected)
    assert matching, (
        f"a validation step running {replacement!r} must be reported for "
        f"omitting {expected}"
    )


@pytest.mark.parametrize(("replacement", "expected"), COMMAND_SCOPED_CASES)
def test_the_detectors_read_the_report_copy_as_one_command(
    replacement: str, expected: str | None
) -> None:
    """Judge the copy per command, not per line.

    A shell line is not a command. A step written as a one-liner joining its
    commands with `&&` or `;` would satisfy a line-wide scan with a copy that
    put something else into some other directory: the match would begin at that
    `cp` and finish at the later command naming the staged directory, and the
    contract would certify a staged directory the report never reached. So the
    scan is scoped to a command segment, and the control case is what keeps it
    from being scoped so finely that a correct one-liner is rejected.
    """
    offenders = _offenders_for(replacement)
    if expected is None:
        assert not offenders, (
            f"a one-liner staging the report with {replacement!r} satisfies "
            f"the contract; it was reported as {offenders}"
        )
        return
    assert _matching(offenders, expected), (
        f"a line naming the report and the staged directory in different "
        f"commands must be reported for omitting {expected}: {offenders}"
    )


@pytest.mark.parametrize(("replacement", "expected"), COPY_DIRECTION_CASES)
def test_the_detectors_read_the_copy_in_the_direction_it_is_written(
    replacement: str, expected: str | None
) -> None:
    """Judge which way round the copy goes, not merely which names are in it.

    `cp` takes its source first. A command naming both the report and the
    staged path reads as correct to a scan that asks only whether both appear,
    and copies the report *out* of the directory the validator is about to
    read. The staged directory would then hold whatever the copy left behind,
    and the contract would certify a report that had just left it.
    """
    offenders = _offenders_for(replacement)
    if expected is None:
        assert not offenders, (
            f"a copy staging the report with {replacement!r} satisfies the "
            f"contract; it was reported as {offenders}"
        )
        return
    assert _matching(offenders, expected), (
        f"a copy reading the report out of the staged directory must be "
        f"reported for omitting {expected}: {offenders}"
    )


@pytest.mark.parametrize(("replacement", "expected"), CREATION_BINDING_CASES)
def test_the_detectors_bind_a_created_directory_to_its_own_name(
    replacement: str, expected: str | None
) -> None:
    """Credit a created directory only to the name that received it.

    A segment can assign several names and create one directory. Read as a set
    of names on the line, every one of them is recorded as made, and a staged
    directory that is a bare literal nothing created then passes the creation
    check on the strength of the neighbour command that made something else —
    the very fault the check was added for.
    """
    offenders = _offenders_for(replacement)
    if expected is None:
        assert not offenders, (
            f"a script creating its staged directory with {replacement!r} "
            f"satisfies the contract; it was reported as {offenders}"
        )
        return
    assert _matching(offenders, expected), (
        f"a directory created for another name must not satisfy the creation "
        f"check: {offenders}"
    )
