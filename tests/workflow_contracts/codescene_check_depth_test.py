"""Forbid repository workflows from running a PR CodeScene coverage check.

Pull requests enforce coverage through the local ratchet. CodeScene receives
only the authoritative report uploaded from `main`, so no workflow should
invoke the shared action in `check` mode. The pure gate-shape predicates
remain property-tested separately because they document the external action's
history requirement should that mode ever be reconsidered.

Reading the workflow files is `workflow_loading.all_workflow_documents`'s
job: it already reads both GitHub extensions and refuses a directory that
would silently yield nothing.

Run via `make test-workflow-contracts`.
"""

from codescene_check_depth_invariants import runs_the_gate
from timeout_budgets import WORKFLOWS_DIRECTORY
from workflow_loading import all_workflow_documents, require_list, require_mapping


def _check_jobs_in(name: str, document: dict[str, object]) -> list[str]:
    """Return the jobs of one workflow that invoke CodeScene in check mode."""
    jobs = require_mapping(document.get("jobs"), f"{name} jobs")
    found: list[str] = []
    for job_name, declaration in jobs.items():
        description = f"{name}:{job_name}"
        job = require_mapping(declaration, description)
        raw_steps = require_list(job.get("steps", []), f"{description} steps")
        if runs_the_gate([step for step in raw_steps if isinstance(step, dict)]):
            found.append(description)
    return found


def _codescene_check_jobs() -> list[str]:
    """Return every workflow job that invokes CodeScene in check mode."""
    documents = all_workflow_documents(WORKFLOWS_DIRECTORY)
    return [
        description
        for name, document in sorted(documents.items())
        for description in _check_jobs_in(name, document)
    ]


def test_no_workflow_runs_a_codescene_coverage_check() -> None:
    """Keep pull-request coverage enforcement inside the local ratchet."""
    found = _codescene_check_jobs()
    assert not found, (
        f"CodeScene check mode must not publish pull-request coverage: {found!r}"
    )
