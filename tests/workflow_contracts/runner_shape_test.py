"""Hold the runner shapes the Linux jobs are sized to.

Placement says which provider runs a job; this module says how large that
runner is and holds the job's own worker bounds to it. Two jobs are escalated
from `ubicloud-standard-2` on evidence rather than intuition, so the same
contracts require the memory measurement that lets the escalation be reviewed
and, if the peak turns out to be modest, reversed.

Run via ``make test-workflow-contracts``.
"""

import pytest
import yaml
from runner_placement_invariants import (
    INSTRUMENTED_BUILD_JOBS,
    LANE_VCPUS,
    UBICLOUD_LABELS,
    is_bounded_worker_count,
)
from workflow_loading import (
    REPO_ROOT,
    job_steps,
    load_workflow,
    require_list,
    require_mapping,
    workflow_job,
)

WORKFLOW_DIR = REPO_ROOT / ".github" / "workflows"
MEMORY_SAMPLER = "./.github/actions/memory-sampler"


def _workflow_env(workflow: dict[str, object]) -> dict[str, object]:
    """Return a workflow's top-level environment mapping."""
    return require_mapping(workflow.get("env", {}), "the workflow env")


def _all_workflow_text() -> str:
    """Return every workflow file's text, concatenated."""
    return "\n".join(
        path.read_text(encoding="utf-8")
        for path in sorted([*WORKFLOW_DIR.glob("*.yml"), *WORKFLOW_DIR.glob("*.yaml")])
    )


UBICLOUD_WORKER_BOUNDS = (
    # The instrumented run reads cargo's and nextest's own variables rather
    # than the Make variables the folded-away test step consumed.
    (
        "ci.yml",
        "build-test",
        ("BUILD_JOBS", "CARGO_BUILD_JOBS", "NEXTEST_TEST_THREADS"),
    ),
    (
        "coverage-main.yml",
        "coverage-upload",
        ("CARGO_BUILD_JOBS", "NEXTEST_TEST_THREADS"),
    ),
    ("netsukefile-test.yml", "netsukefile", ("BUILD_JOBS",)),
)


@pytest.mark.parametrize(
    ("workflow_name", "job_name"),
    [
        ("ci.yml", "build-test"),
        ("ci.yml", "kani-smoke"),
        ("coverage-main.yml", "coverage-upload"),
        ("netsukefile-test.yml", "netsukefile"),
    ],
)
def test_every_ubicloud_job_declares_a_timeout(
    workflow_name: str, job_name: str
) -> None:
    """Require a timeout so a stuck Ubicloud VM cannot bill indefinitely."""
    job = workflow_job(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    timeout = job.get("timeout-minutes")
    assert isinstance(timeout, int), (
        f"{workflow_name} job {job_name} must set timeout-minutes, got {timeout!r}"
    )
    assert timeout > 0, (
        f"{workflow_name} job {job_name} must set a positive timeout, got {timeout!r}"
    )


@pytest.mark.parametrize(
    ("workflow_name", "job_name", "flag_names"), UBICLOUD_WORKER_BOUNDS
)
def test_worker_counts_match_the_lane_vcpu_count(
    workflow_name: str, job_name: str, flag_names: tuple[str, ...]
) -> None:
    """Keep compilation and test workers within the placed shape's vCPUs."""
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    job = workflow_job(workflow, job_name)
    runner = str(job.get("runs-on"))
    assert runner in LANE_VCPUS, (
        f"{workflow_name} job {job_name} runs on {runner!r}, whose vCPU count "
        "this suite does not know; add it to LANE_VCPUS"
    )
    vcpus = LANE_VCPUS[runner]
    env = require_mapping(job.get("env"), f"jobs.{job_name}.env")
    missing = [name for name in flag_names if name not in env]
    assert not missing, (
        f"{workflow_name} job {job_name} must declare {missing!r}; a missing "
        "bound is a contract failure, not a KeyError"
    )
    flags = {name: str(env[name]) for name in flag_names}
    assert is_bounded_worker_count(vcpus, flags), (
        f"{workflow_name} job {job_name} runs on {runner} with {vcpus} vCPUs "
        f"but declares {flags!r}"
    )
    declared = env.get("LINUX_LANE_VCPUS") or _workflow_env(workflow).get(
        "LINUX_LANE_VCPUS"
    )
    assert str(declared) == str(vcpus), (
        f"{workflow_name} job {job_name} must name its vCPU count once, "
        f"got {declared!r}"
    )


def test_windows_lane_names_its_vcpu_count_once() -> None:
    """Derive the Windows worker counts from one named constant."""
    workflow = load_workflow(WORKFLOW_DIR / "ci-windows.yml")
    vcpus = LANE_VCPUS["windows-latest"]
    assert str(_workflow_env(workflow).get("WINDOWS_LANE_VCPUS")) == str(vcpus), (
        "ci-windows.yml must declare the windows-latest vCPU count once"
    )
    job = workflow_job(workflow, "build-test-windows")
    env = require_mapping(job.get("env"), "jobs.build-test-windows.env")
    flags = {
        name: str(env[name])
        for name in ("BUILD_JOBS", "NEXTEST_BUILD_JOBS", "NEXTEST_TEST_JOBS")
    }
    assert is_bounded_worker_count(vcpus, flags), (
        f"build-test-windows declares {flags!r} for a {vcpus} vCPU runner"
    )


def test_actionlint_registers_exactly_the_ubicloud_labels_in_use() -> None:
    """Register every intentional Ubicloud label, and nothing else.

    actionlint rejects an unregistered self-hosted label, so a typo or an
    unreviewed shape fails the lint gate instead of queueing forever.
    """
    config = yaml.safe_load(
        (REPO_ROOT / ".github" / "actionlint.yaml").read_text(encoding="utf-8")
    )
    registered = require_mapping(config, "actionlint config")["self-hosted-runner"]
    labels = tuple(
        str(label)
        for label in require_list(
            require_mapping(registered, "self-hosted-runner").get("labels"),
            "self-hosted-runner labels",
        )
    )
    assert sorted(labels) == sorted(UBICLOUD_LABELS), (
        f"actionlint must register exactly {UBICLOUD_LABELS!r}, got {labels!r}"
    )
    workflow_text = _all_workflow_text()
    for label in labels:
        assert label in workflow_text, f"{label} is registered but never used"


@pytest.mark.parametrize(("workflow_name", "job_name"), INSTRUMENTED_BUILD_JOBS)
def test_escalated_jobs_measure_the_memory_that_escalated_them(
    workflow_name: str, job_name: str
) -> None:
    """Require both escalated jobs to sample memory and publish the peak.

    The escalation from `ubicloud-standard-2` rests on an inference: a runner
    vanished mid-build with no log. Without the measurement the shape can
    never be reviewed, so the sampler is part of the escalation rather than a
    convenience.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    modes = [
        require_mapping(step.get("with"), f"{job_name} sampler inputs").get("mode")
        for step in steps
        if str(step.get("uses", "")) == "./.github/actions/memory-sampler"
    ]
    assert modes == ["start", "report"], (
        f"{workflow_name} {job_name} must start the sampler and report its "
        f"peak, in that order; got {modes!r}"
    )
    report = next(
        step
        for step in steps
        if str(step.get("uses", "")) == "./.github/actions/memory-sampler"
        and require_mapping(step.get("with"), "sampler inputs").get("mode") == "report"
    )
    assert report.get("if") == "always()", (
        f"{workflow_name} {job_name} must report the peak even when the job fails"
    )


def test_both_instrumented_jobs_share_one_lane_size() -> None:
    """Keep the two instrumented jobs on the same shape.

    They run the same workload, so a shape change that reached only one of
    them would leave the other with the failure the escalation addressed.
    """
    sizes = {
        f"{workflow_name} {job_name}": str(
            require_mapping(
                workflow_job(load_workflow(WORKFLOW_DIR / workflow_name), job_name).get(
                    "env"
                ),
                f"{job_name} env",
            ).get("LINUX_LANE_VCPUS")
        )
        for workflow_name, job_name in INSTRUMENTED_BUILD_JOBS
    }
    assert len(set(sizes.values())) == 1, (
        f"the instrumented jobs must share one lane size, got {sizes!r}"
    )
