"""Hold the resolver ahead of every step that reads the pinned toolchain.

``NETSUKE_RUST_TOOLCHAIN`` is exported by ``.github/actions/resolve-toolchain``
from ``rust-toolchain.toml``. The ``env`` context resolves per job and per step,
so a step that reads the variable before the job has run the resolver gets the
empty string, and ``setup-rust`` would then take its rust-toolchain-file arm.
These tests read every workflow, so a job added later is held to the same order
without being listed here.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from workflow_loading import WORKFLOW_DIR, all_workflow_documents

RESOLVER = "./.github/actions/resolve-toolchain"
VARIABLE = "NETSUKE_RUST_TOOLCHAIN"


def _jobs() -> list[tuple[str, str, dict[str, object]]]:
    """Return every job of every workflow as (file, job id, body)."""
    found = []
    for name, document in all_workflow_documents(WORKFLOW_DIR).items():
        jobs = document.get("jobs")
        if not isinstance(jobs, dict):
            continue
        found.extend(
            (name, job_id, body)
            for job_id, body in jobs.items()
            if isinstance(body, dict)
        )
    return found


def _steps(body: dict[str, object]) -> list[dict[str, object]]:
    """Return a job's steps, or none for a job that calls a reusable workflow."""
    steps = body.get("steps")
    return (
        [step for step in steps if isinstance(step, dict)]
        if isinstance(steps, list)
        else []
    )


def _reads_variable(step: dict[str, object]) -> bool:
    """Whether the step names the variable in any of its inputs or script."""
    return VARIABLE in str(step.get("with", "")) + str(step.get("run", "")) + str(
        step.get("env", "")
    )


READERS = [
    pytest.param(name, job_id, body, id=f"{name}:{job_id}")
    for name, job_id, body in _jobs()
    if any(_reads_variable(step) for step in _steps(body))
]


def test_some_jobs_read_the_pinned_toolchain() -> None:
    """Guard against the scan matching nothing and passing vacuously."""
    assert len(READERS) >= 8, f"expected the toolchain readers, found {len(READERS)}"


@pytest.mark.parametrize(("name", "job_id", "body"), READERS)
def test_the_resolver_runs_before_every_reader(
    name: str, job_id: str, body: dict[str, object]
) -> None:
    """The job runs the resolver, and before any step that reads the variable."""
    steps = _steps(body)
    resolver = [i for i, step in enumerate(steps) if step.get("uses") == RESOLVER]
    assert resolver, f"{name} job {job_id} reads {VARIABLE} but never resolves it"
    first = resolver[0]
    early = [
        step.get("name") or step.get("uses")
        for step in steps[:first]
        if _reads_variable(step)
    ]
    assert not early, (
        f"{name} job {job_id} reads {VARIABLE} before the resolver: {early}"
    )
    assert VARIABLE not in typ.cast("dict", body.get("env") or {}), (
        f"{name} job {job_id} declares {VARIABLE} instead of resolving it"
    )
