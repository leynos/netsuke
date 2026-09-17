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

The same revision carries the publication opt-out, so this is also where the
two lanes are held apart on it: the pull-request lane declines the action's own
archive, and the trunk lane keeps the default so it still publishes the report
CodeScene reads. Setting it in the wrong lane would either publish a
pull-request report or stop the trunk upload, and neither is visible from a
green run.

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

#: The revision that guards the baseline save on a push to refs/heads/main and
#: carries the publication opt-out the pull-request lane passes. The guarantee
#: arrived in a particular revision, so a bump has to update this constant and
#: make someone confirm the new one still keeps a pull request from publishing.
GENERATE_COVERAGE: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/generate-coverage@"
    "a5765019912a8ab6882b12db049c7cde635f3a85"
)

#: The input that suppresses the action's own archive step. A pull-request
#: caller sets it; the trunk caller must not, or CodeScene is sent nothing.
PUBLICATION_OPT_OUT_INPUT: typ.Final[str] = "publish-artefact"

#: The job that measures a pull request and the job that publishes the trunk
#: report. Only the first declines the archive.
PULL_REQUEST_JOB: typ.Final[tuple[Path, str]] = (CI_WORKFLOW_PATH, "build-test")
TRUNK_JOB: typ.Final[tuple[Path, str]] = (
    COVERAGE_MAIN_WORKFLOW_PATH,
    "coverage-upload",
)

#: Every job invoking the shared coverage action, and the workflow declaring it.
COVERAGE_JOBS: typ.Final[tuple[tuple[Path, str], ...]] = (
    PULL_REQUEST_JOB,
    TRUNK_JOB,
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


def _inputs(workflow_path: Path, job_name: str) -> dict[str, object]:
    """Return one coverage step's ``with`` block."""
    return require_mapping(
        _coverage_step(workflow_path, job_name).get("with"),
        f"{workflow_path.name}:{job_name} coverage step inputs",
    )


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
    inputs = _inputs(workflow_path, job_name)

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


def test_the_pull_request_lane_declines_the_archive_the_trunk_lane_keeps() -> None:
    """Split the two lanes on the action's own archive step.

    The action archives the report it generated unless the caller passes the
    opt-out, and that step belongs to the action rather than to either
    workflow, so no scan of this repository's steps can see it. Both
    directions matter: a pull-request lane that omitted the opt-out would
    publish the report the local boundary exists to keep local, and a trunk
    lane that passed it would leave CodeScene with no report to read.
    """
    pull_request_inputs = _inputs(*PULL_REQUEST_JOB)
    assert pull_request_inputs.get(PUBLICATION_OPT_OUT_INPUT) == "false", (
        f"the pull-request lane must pass "
        f"{PUBLICATION_OPT_OUT_INPUT}=false, got "
        f"{pull_request_inputs.get(PUBLICATION_OPT_OUT_INPUT)!r}; the coverage "
        f"action otherwise archives the report it generated"
    )

    trunk_inputs = _inputs(*TRUNK_JOB)
    assert PUBLICATION_OPT_OUT_INPUT not in trunk_inputs, (
        f"the trunk lane sets {PUBLICATION_OPT_OUT_INPUT}="
        f"{trunk_inputs.get(PUBLICATION_OPT_OUT_INPUT)!r}; that suppresses the "
        f"only upload CodeScene reads"
    )
