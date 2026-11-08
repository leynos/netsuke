"""Prove a release job re-run cannot collide with its own earlier telemetry.

A job re-run keeps the run id and increments only the attempt. An artefact
named for the job alone therefore collides with the first attempt's, and
``actions/upload-artifact`` v7 refuses to overwrite an existing artefact rather
than replacing it. Resolving that with ``overwrite: true`` would be worse: the
whole purpose of the retained record is to let an operator compare a re-run
against the attempt that failed, and overwriting deletes the comparison.

This file pins the property the name has to have, independently of which jobs
currently upload telemetry. :func:`test_a_rerun_keeps_its_own_telemetry`
compares two attempts of the *same* job, which is the only comparison that can
distinguish a job-scoped name from a job-and-attempt-scoped one; a check that
merely asserted the run id appeared would pass for both.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

import pytest
from release_operation_observability_support import (
    OBSERVABILITY_ARTIFACT_STEP,
    named_step,
)
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    require_mapping,
)

if typ.TYPE_CHECKING:
    import pathlib

#: The events GitHub substitutes into a workflow expression. Only ``run_id`` and
#: ``run_attempt`` distinguish one attempt of a job from another, so any other
#: context value would leave a re-run colliding.
_ATTEMPT_CONTEXT = "${{ github.run_attempt }}"

#: A GitHub expression, so a name can be rendered per attempt without guessing
#: how the workflow spells its separators.
_EXPRESSION = re.compile(r"\$\{\{\s*([^}]+?)\s*\}\}")


@pytest.fixture(scope="module")
def release_workflow() -> dict[str, object]:
    """Load the release workflow once for the rerun assertions."""
    return load_workflow(RELEASE_WORKFLOW_PATH)


def _telemetry_jobs(workflow: dict[str, object]) -> list[str]:
    """Return every job that retains an observability artefact.

    Returns
    -------
    list[str]
        Job names, so a case names a real job rather than a literal.

    Notes
    -----
    Discovered rather than listed. A job that starts retaining telemetry in
    future joins these assertions without anyone remembering to add it, which
    is the only way a sweep like this stays true.
    """
    jobs = require_mapping(workflow.get("jobs"), "the release jobs")
    # A job whose steps are not a list declares no upload to find, rather than
    # failing the sweep: the step-shape contract is :mod:`workflow_loading`'s
    # to state, and repeating it here would make one malformed job hide every
    # other job's telemetry name.
    return [
        name
        for name in jobs
        if isinstance((declaration := jobs[name]), dict)
        and isinstance(steps := declaration.get("steps"), list)
        and any(
            isinstance(step, dict) and step.get("name") == OBSERVABILITY_ARTIFACT_STEP
            for step in steps
        )
    ]


def _render(template: str, *, job: str, run_id: str, attempt: str) -> str:
    """Render a name as GitHub would for one attempt of one job.

    Parameters
    ----------
    template
        The artefact name, with or without ``${{ }}`` expressions.
    job
        The job id the workflow reports as ``github.job``.
    run_id
        The run id shared by every attempt.
    attempt
        The attempt number, which a re-run increments.

    Returns
    -------
    str
        The name the upload would use, with every expression substituted.

    Notes
    -----
    An expression this helper does not know is left literal rather than
    guessed at. That keeps an unknown context value visible to the assertions
    below instead of silently rendering it into something that happens to
    differ per job.
    """
    known = {
        "github.job": job,
        "github.run_id": run_id,
        "github.run_attempt": attempt,
    }
    return _EXPRESSION.sub(
        lambda match: known.get(match.group(1), match.group(0)), template
    )


def _artefact_name(workflow: dict[str, object], job: str) -> str:
    """Return the observability artefact name a job's upload declares.

    Returns
    -------
    str
        The ``name`` input, unrendered.

    Raises
    ------
    AssertionError
        If the job has no telemetry upload or declares no name, which would make
        every rerun assertion below vacuously pass.
    """
    step = named_step(job_steps(workflow, job), OBSERVABILITY_ARTIFACT_STEP)
    with_ = step.get("with")
    assert isinstance(with_, dict), f"{job} telemetry upload must declare inputs"
    name = with_.get("name")
    assert isinstance(name, str) and name, (
        f"{job} telemetry upload must declare a non-empty artefact name"
    )
    return name


def test_release_workflow_retains_telemetry_somewhere(
    release_workflow: dict[str, object],
) -> None:
    """Require the sweep below to have something to sweep.

    Without this the rerun property would hold vacuously over an empty job set,
    which is how an artefact naming regression could land unnoticed.
    """
    jobs = _telemetry_jobs(release_workflow)
    assert jobs, "no release job retains observability telemetry"


def test_every_retaining_job_keeps_its_own_telemetry_across_a_rerun(
    release_workflow: dict[str, object],
) -> None:
    """Give each attempt of every retaining job a distinct artefact name.

    Each comparison is between two attempts of the *same* job and run, so a
    name that carries the job id but not the attempt renders identically for
    both and this test names that job.
    """
    colliding = [
        job
        for job in _telemetry_jobs(release_workflow)
        if _render(
            _artefact_name(release_workflow, job), job=job, run_id="9001", attempt="1"
        )
        == _render(
            _artefact_name(release_workflow, job), job=job, run_id="9001", attempt="2"
        )
    ]
    assert not colliding, (
        f"these jobs' telemetry names do not change across a re-run: {colliding}"
    )


def test_distinct_jobs_never_share_a_telemetry_name(
    release_workflow: dict[str, object],
) -> None:
    """Keep one job's telemetry distinguishable from another's.

    The attempt suffix must not have replaced the job scope: two jobs in one
    attempt uploading to the same name would collide the other way.
    """
    jobs = _telemetry_jobs(release_workflow)
    rendered = {
        job: _render(
            _artefact_name(release_workflow, job), job=job, run_id="9001", attempt="1"
        )
        for job in jobs
    }
    assert len(set(rendered.values())) == len(rendered), (
        f"telemetry names collide between jobs: {rendered}"
    )


def test_the_attempt_suffix_is_what_the_release_workflow_uses(
    release_workflow: dict[str, object],
) -> None:
    """Require the attempt expression itself, not an accidental substitute.

    A name could differ between attempts by embedding something unrelated, and
    then it would differ for the wrong reason -- or stop differing once that
    value stabilised. Naming the expression makes the mechanism explicit.
    """
    jobs = _telemetry_jobs(release_workflow)
    missing = [
        job
        for job in jobs
        if _ATTEMPT_CONTEXT not in _artefact_name(release_workflow, job)
    ]
    assert not missing, (
        f"these jobs' telemetry names omits {_ATTEMPT_CONTEXT}: {missing}"
    )
