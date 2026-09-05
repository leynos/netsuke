"""Contract for which runs may publish the coverage ratchet baseline.

A ratchet is only a ratchet while the baseline it compares against comes from
somewhere a pull request cannot reach. Before the pinned revision the shared
action published from every run that reached its save step, so a pull request
advanced the baseline it was then measured against, and a warm-run dispatch of
``coverage-main.yml`` replaced the very generation it was measuring, despite
that workflow's header describing itself as a reader.

None of that is visible from a green run. A ratchet comparing each pull request
against itself passes exactly as one comparing against trunk does, and it goes
on passing while coverage falls.

The pin is therefore asserted by value here, unlike the shape sweep in
``tests/workflow_shared_actions_pins.rs``, which deliberately lets Dependabot
own each SHA. The guarantee arrived in a particular revision, so a bump has to
update this constant and make someone confirm the new one still keeps a pull
request from publishing.

This repository needs no opt-in. Both workflows run on pushes to ``main``,
which is the trigger the action's guard names, so neither sets
``publish-baseline``.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from workflow_loading import (
    CI_WORKFLOW_PATH,
    COVERAGE_MAIN_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    require_mapping,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

#: The revision that guards the baseline save on a push to refs/heads/main.
GENERATE_COVERAGE = (
    "leynos/shared-actions/.github/actions/generate-coverage@"
    "77ea10341249024e22ec5d9069e3caa7596e0d4f"
)

#: Every job invoking the shared coverage action, and the workflow declaring it.
COVERAGE_JOBS = (
    (CI_WORKFLOW_PATH, "build-test"),
    (COVERAGE_MAIN_WORKFLOW_PATH, "coverage-upload"),
)


def _coverage_step(workflow_path: Path, job_name: str) -> dict[str, object]:
    """Return the single shared-coverage step of a job."""
    workflow = load_workflow(workflow_path)
    matches = [
        step
        for step in job_steps(workflow, job_name)
        if "generate-coverage@" in str(step.get("uses") or "")
    ]
    assert len(matches) == 1, (
        f"{workflow_path.name}:{job_name} must invoke the coverage action "
        f"exactly once, found {len(matches)}"
    )
    return matches[0]


@pytest.mark.parametrize(("workflow_path", "job_name"), COVERAGE_JOBS)
def test_coverage_is_pinned_to_the_guarded_revision(
    workflow_path: Path, job_name: str
) -> None:
    """Both callers must share the revision that carries the guard.

    Two lanes on different revisions would be worse than one stale pin: the
    behaviour would depend on which lane a reader happened to check.
    """
    step = _coverage_step(workflow_path, job_name)

    assert step.get("uses") == GENERATE_COVERAGE, (
        f"{workflow_path.name}:{job_name} must pin {GENERATE_COVERAGE}, got "
        f"{step.get('uses')!r}"
    )


@pytest.mark.parametrize(("workflow_path", "job_name"), COVERAGE_JOBS)
def test_no_caller_opts_out_of_the_guard(workflow_path: Path, job_name: str) -> None:
    """Leaving ``publish-baseline`` unset is what keeps a pull request out.

    Setting it to ``always`` would restore exactly the behaviour this pin
    exists to remove. Neither workflow needs it: both run on pushes to
    ``main``, which is what the guard admits.
    """
    step = _coverage_step(workflow_path, job_name)
    inputs = require_mapping(
        step.get("with"), f"{workflow_path.name}:{job_name} coverage step inputs"
    )

    assert inputs.get("with-ratchet") == "true", (
        f"{workflow_path.name}:{job_name} must enable the ratchet, or the "
        f"guard governs nothing, got {inputs.get('with-ratchet')!r}"
    )
    assert "publish-baseline" not in inputs, (
        f"{workflow_path.name}:{job_name} sets publish-baseline="
        f"{inputs.get('publish-baseline')!r}; a run that is not a trunk push "
        f"would then advance the baseline it is measured against"
    )


@pytest.mark.parametrize(("workflow_path", "job_name"), COVERAGE_JOBS)
def test_the_trunk_push_trigger_stays_on_main(
    workflow_path: Path, job_name: str
) -> None:
    """The guard admits a push to main, so that trigger must exist and be narrow.

    Without it no run could publish and the baseline would stop advancing;
    widened past ``main`` it would let another branch publish.
    """
    workflow = load_workflow(workflow_path)
    triggers = require_mapping(workflow["on"], f"{workflow_path.name}.on")
    push = require_mapping(triggers["push"], f"{workflow_path.name}.on.push")

    assert push.get("branches") == ["main"], (
        f"{workflow_path.name} must trigger on pushes to main only, got "
        f"{push.get('branches')!r}"
    )
