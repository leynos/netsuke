"""Forbid repository workflows from running a PR CodeScene coverage check.

Pull requests enforce coverage through the local ratchet. CodeScene receives
only the authoritative report uploaded from `main`, so no workflow should
invoke the shared action in `check` mode. The pure gate-shape predicates
remain property-tested separately because they document the external action's
history requirement should that mode ever be reconsidered.

Run via `make test-workflow-contracts`.
"""

from codescene_check_depth_invariants import runs_the_gate
from workflow_loading import REPO_ROOT, load_workflow, require_list, require_mapping


def _codescene_check_jobs() -> list[str]:
    """Return every workflow job that invokes CodeScene in check mode."""
    workflow_dir = REPO_ROOT / ".github" / "workflows"
    found: list[str] = []
    for pattern in ("*.yml", "*.yaml"):
        for path in sorted(workflow_dir.glob(pattern)):
            jobs = require_mapping(load_workflow(path).get("jobs"), f"{path.name} jobs")
            for name, declaration in jobs.items():
                description = f"{path.name}:{name}"
                job = require_mapping(declaration, description)
                raw_steps = require_list(job.get("steps", []), f"{description} steps")
                steps = [step for step in raw_steps if isinstance(step, dict)]
                if runs_the_gate(steps):
                    found.append(description)
    return found


def test_no_workflow_runs_a_codescene_coverage_check() -> None:
    """Keep pull-request coverage enforcement inside the local ratchet."""
    assert not _codescene_check_jobs(), (
        "CodeScene check mode must not publish pull-request coverage: "
        f"{_codescene_check_jobs()!r}"
    )
