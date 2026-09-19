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
from fork_fallback import owned_runner, read_placement
from runner_placement_invariants import (
    GITHUB_HOSTED_LABELS,
    INSTRUMENTED_BUILD_JOBS,
    LANE_VCPUS,
    UBICLOUD_DEFAULT_LABEL,
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


def _all_jobs() -> list[tuple[str, str, dict[str, object]]]:
    """Return every job in every workflow, with the file that declares it."""
    found: list[tuple[str, str, dict[str, object]]] = []
    for path in sorted([*WORKFLOW_DIR.glob("*.yml"), *WORKFLOW_DIR.glob("*.yaml")]):
        jobs = load_workflow(path).get("jobs")
        if not isinstance(jobs, dict):
            continue
        found.extend(
            (path.name, str(job_name), job)
            for job_name, job in jobs.items()
            if isinstance(job, dict)
        )
    return found


def is_self_hosted_label(label: str) -> bool:
    """Return whether a label names a runner GitHub does not host.

    By name, not by prefix. A prefix test absorbs any new label that looks
    hosted, so a lane moved onto an unknown image would drop out of "in use"
    and its registration would go unnoticed.

    Returns
    -------
    bool
        True when the label is not one of GitHub's own.
    """
    return label not in GITHUB_HOSTED_LABELS


def _selected_labels(declaration: object) -> list[str]:
    """Return the labels one `runs-on` value can select."""
    # Both arms of a conditional count: a job that falls back for forks may
    # run on either, and reading only the declaration would take the whole
    # expression for one unrecognized label.
    placement = read_placement(declaration)
    if placement is not None:
        return [placement.fork, placement.owned]
    match declaration:
        case str() as label:
            return [label]
        case list() as entries:
            return [str(entry) for entry in entries]
        case _:
            return []


def _matrix_runners(job: dict[str, object]) -> set[str]:
    """Return every runner a job's matrix selects through a `runner` entry."""
    # The macOS lanes choose their image this way, and a label smuggled into a
    # matrix is still a label in use.
    strategy = job.get("strategy")
    matrix = strategy.get("matrix") if isinstance(strategy, dict) else None
    includes = (matrix or {}).get("include") or []
    return {
        str(item["runner"])
        for item in includes
        if isinstance(item, dict) and "runner" in item
    }


def _self_hosted_labels_in_use() -> set[str]:
    """Return every label the workflows select that GitHub does not host."""
    # A matrix `runner` value counts too, because the macOS lanes select their
    # image that way and a label smuggled into a matrix is still one in use.
    found: set[str] = set()
    for _, _, job in _all_jobs():
        found.update(_selected_labels(job.get("runs-on")))
        found.update(_matrix_runners(job))
    return {label for label in found if is_self_hosted_label(label)}


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


def test_release_linux_package_job_declares_a_timeout() -> None:
    """Require the caller-selected Linux package job to bound its runtime.

    `release.yml`'s `build-linux` job calls the reusable
    `build-and-package.yml` workflow and passes `runner: UBICLOUD_DEFAULT_LABEL`
    as its `runner` input, so the reusable job's `runs-on:
    ${{ inputs.runner }}` resolves to a Ubicloud shape here even though its own
    YAML names no literal label. The parametrised jobs above only enumerate
    jobs with a literal Ubicloud label in their own `runs-on`, so this
    caller-selected placement needs its own contract: without it, deleting
    `timeout-minutes` from the reusable `build` job would leave a stuck
    Ubicloud VM billing indefinitely with nothing here to catch it.
    """
    release_workflow = load_workflow(WORKFLOW_DIR / "release.yml")
    release_job = workflow_job(release_workflow, "build-linux")
    inputs = require_mapping(release_job.get("with"), "build-linux with")
    assert inputs.get("runner") == UBICLOUD_DEFAULT_LABEL, (
        "release.yml build-linux must pass the default Ubicloud label to "
        f"build-and-package.yml, got {inputs.get('runner')!r}"
    )
    build_job = workflow_job(
        load_workflow(WORKFLOW_DIR / "build-and-package.yml"), "build"
    )
    timeout = build_job.get("timeout-minutes")
    assert isinstance(timeout, int), (
        "build-and-package.yml build must set timeout-minutes, since "
        f"release.yml selects it with {UBICLOUD_DEFAULT_LABEL!r}, got {timeout!r}"
    )
    assert timeout > 0, (
        f"build-and-package.yml build must set a positive timeout, got {timeout!r}"
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
    # The owned arm, not the declaration: a fork's run is GitHub-hosted and its
    # shape is not what these worker bounds are derived from.
    runner = owned_runner(job.get("runs-on"))
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
    """Register every self-hosted label in use, and nothing else.

    Equality in both directions. actionlint rejects an unregistered
    self-hosted label, so a typo or an unreviewed shape fails the lint gate
    instead of queueing forever; and a registration left behind after a lane
    moved back to GitHub's pool hides a runner assignment already retired.

    "In use" is derived from the workflows rather than read from the reviewed
    constant, and the two are compared separately, so a lane that quietly
    stops using a shape fails rather than agreeing with a constant nobody
    revisited. The previous form asked only whether each registered label
    appeared anywhere in the concatenated workflow text, which a mention in a
    comment satisfies.
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
    in_use = _self_hosted_labels_in_use()
    assert sorted(labels) == sorted(in_use), (
        f"actionlint must register exactly the self-hosted labels the "
        f"workflows select. It registers {sorted(labels)!r} and they select "
        f"{sorted(in_use)!r}"
    )
    assert sorted(in_use) == sorted(UBICLOUD_LABELS), (
        f"the reviewed label set and the labels in use have diverged: "
        f"{sorted(UBICLOUD_LABELS)!r} against {sorted(in_use)!r}. A shape "
        f"change belongs in both, with the measurement that justifies it"
    )


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


@pytest.mark.parametrize(
    ("label", "self_hosted"),
    [
        pytest.param("ubicloud-standard-2-ubuntu-2404", True, id="a-label-in-use"),
        pytest.param("ubuntu-latest", False, id="a-named-hosted-label"),
        pytest.param("macos-15-intel", False, id="another-named-one"),
        # The case that separates a named set from a prefix test: a hosted
        # family, a label this repository does not use, and one that must
        # therefore be reported rather than silently excused.
        pytest.param("ubuntu-20.04", True, id="a-hosted-family-member-not-named"),
        pytest.param(
            "${{ inputs.runner }}",
            False,
            id="a-caller-supplied-runner-names-no-label-here",
        ),
    ],
)
def test_the_registry_reads_hosted_labels_by_name(
    label: str, *, self_hosted: bool
) -> None:
    """The registry question asks by name, and the two readings differ.

    Over this repository's own workflows a prefix test and the named set agree
    exactly, so the derivation above cannot tell them apart. These cases can.
    """
    assert is_self_hosted_label(label) is self_hosted, (
        f"`{label}` must be classified by name for the registry question"
    )
