"""The fork arm on the lanes that serve pull requests, and its absence elsewhere.

A pull request from a fork cannot obtain a Ubicloud runner. Without the arm the
lane never starts, and the branch ruleset waits on a required check that will
not report, which presents as a stuck pull request rather than as a placement
fault.

These read the checked-in workflows. The reading itself is exercised here too,
because this repository declares only correct workflows: parametrised over them
a reader that returned nothing would agree with the tree exactly as well as one
that read correctly.

Run via ``make test-workflow-contracts``.
"""

import pytest
from fork_fallback import (
    FORK_FALLBACK_RUNNER,
    FORK_GUARD,
    Placement,
    fork_fallback_offences,
    owned_runner,
    read_placement,
)
from runner_placement_test import DIRECT_RUNNER_SOURCES, WORKFLOW_DIR
from workflow_loading import load_workflow, workflow_job

#: A declaration of the deployed shape, over an arbitrary owned label.
DEPLOYED = (
    f"${{{{ {FORK_GUARD} && '{FORK_FALLBACK_RUNNER}' "
    "|| 'ubicloud-standard-2-ubuntu-2404' }}"
)


def _checked_in_declarations() -> dict[str, object]:
    """Return each directly placed job's `runs-on` exactly as written.

    Typed as `object` rather than `str`: a job may declare a label sequence, a
    mapping, or nothing at all, and narrowing here would put the reader's
    fail-closed cases out of reach of the contract that uses them.

    Returns
    -------
    dict[str, object]
        Assignment key to the parsed ``runs-on`` value, unnormalised.
    """
    return {
        key: workflow_job(load_workflow(WORKFLOW_DIR / workflow), job).get("runs-on")
        for key, workflow, job in DIRECT_RUNNER_SOURCES
    }


def test_every_lane_declares_the_arm_it_should_and_no_other() -> None:
    """Both directions, over every directly placed job in the estate.

    A pull-request lane missing the arm is the fault this exists to prevent. A
    lane no fork reaches carrying one is the fault that follows from fixing the
    first by imitation, so it is an offence too.
    """
    offences = fork_fallback_offences(_checked_in_declarations())
    assert not offences, (
        f"these lanes declare the wrong placement: {offences}; a lane serving "
        f"pull requests must branch on {FORK_GUARD} to {FORK_FALLBACK_RUNNER}, "
        f"and every other lane must name its runner outright"
    )


def test_no_runs_on_declaration_carries_a_line_break() -> None:
    """A folded scalar can keep its break, and GitHub evaluates it anyway.

    A continuation indented deeper than its `runs-on:` key is a more-indented
    line inside a folded block, so YAML keeps the newline rather than folding
    it to a space, and the expression arrives with a break inside it. The lane
    still runs, so a green run is not evidence that the declaration is well
    formed. This is the only place that reads it.
    """
    broken = [
        key
        for key, declaration in _checked_in_declarations().items()
        if isinstance(declaration, str) and "\n" in declaration
    ]
    assert not broken, (
        f"a runs-on must parse to one line: {broken}; keep a folded scalar's "
        f"continuation at the same indent as its first line"
    )


@pytest.mark.parametrize(
    ("declaration", "expected"),
    [
        pytest.param(
            DEPLOYED,
            Placement(
                FORK_GUARD, FORK_FALLBACK_RUNNER, "ubicloud-standard-2-ubuntu-2404"
            ),
            id="the-deployed-shape",
        ),
        pytest.param(
            "${{   a.b   &&   'x'   ||   'y'   }}",
            Placement("a.b", "x", "y"),
            id="generous-internal-spacing",
        ),
        pytest.param("ubuntu-latest", None, id="a-literal-label"),
        pytest.param("${{ matrix.os }}", None, id="a-matrix-reference"),
        pytest.param("${{ inputs.runner }}", None, id="a-caller-supplied-runner"),
        pytest.param("${{ a.b && 'x' }}", None, id="a-single-armed-expression"),
        pytest.param("${{ !a.b && 'x' || 'y' }}", None, id="a-negated-guard"),
        pytest.param("${{ a.b == 'c' && 'x' || 'y' }}", None, id="a-compared-guard"),
        pytest.param("${{ a.b && x || 'y' }}", None, id="an-unquoted-arm"),
        pytest.param("${{ a.b\n&& 'x' || 'y' }}", None, id="a-line-break-inside-it"),
        pytest.param(["a", "b"], None, id="a-label-sequence"),
        pytest.param(None, None, id="no-declaration-at-all"),
    ],
)
def test_the_reader_accepts_one_shape_and_refuses_the_rest(
    declaration: object, expected: Placement | None
) -> None:
    """Anything but the prescribed spelling must read as no placement.

    The estate writes one spelling. A negated guard or a comparison says the
    same thing with the arms the other way round, and reading either as the
    prescribed form would let two spellings of one placement drift apart while
    both satisfied the contract.

    The line-break case is the one that matters most: it must read as no
    placement here, so the assertion written for it reports it rather than this
    reader quietly parsing through it.
    """
    assert read_placement(declaration) == expected, (
        f"{declaration!r} must read as {expected!r}; the estate writes one "
        f"spelling and anything else is refused where it is found"
    )


@pytest.mark.parametrize(
    ("declaration", "expected"),
    [
        pytest.param(DEPLOYED, "ubicloud-standard-2-ubuntu-2404", id="the-owned-arm"),
        pytest.param("windows-latest", "windows-latest", id="a-literal-label"),
        pytest.param(
            "${{ inputs.runner }}",
            "${{ inputs.runner }}",
            id="a-caller-supplied-runner-is-left-alone",
        ),
    ],
)
def test_the_owned_arm_is_what_every_sizing_rule_reads(
    declaration: object, expected: str
) -> None:
    """The vCPU table and the assignment table both ask this question.

    A fork's run is a GitHub-hosted fallback whose shape those rules
    deliberately do not govern, so normalising here keeps one reading of the
    declaration rather than one per caller. A value that is not a placement
    must pass through untouched, or every literal lane would be renamed.
    """
    assert owned_runner(declaration) == expected, (
        f"{declaration!r} must normalise to {expected!r}; every sizing rule "
        f"reads this, and a value that is not a placement passes through"
    )
