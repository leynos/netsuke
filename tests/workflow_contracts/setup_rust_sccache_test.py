"""Hold `setup-rust` as the single owner of the Linux lanes' compiler cache.

shared-actions ADR 0005 makes `setup-rust` select sccache's backend from the
runner: on Ubicloud it exports the cache-proxy credentials, clears the v2
switch the proxy does not serve, and points sccache at the proxy; on a
GitHub-hosted runner it picks local disk. The Linux lanes carried their own
version of that by hand, through a credentials action, a job-level wrapper and
backend switch, a pinned sccache installer, a statistics reset, and a
local-directory fallback behind a repository variable. Every one of those
pieces would now override the action's choice without a word, because a
caller's wrapper, directory or switch wins over it.

The rules: each Linux lane calls a pinned `setup-rust` once with sccache on,
an id its report can read, and the `expect-cache` its placement allows; no
retired piece survives anywhere in those lanes or in the gate cache they
share; and the statistics report follows the last compile and names the
backend, because `Cache location` reads `ghac` for the proxy and GitHub's own
service alike.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

import pytest
from cache_contract_data import (
    ACTION_DIR,
    HAND_ROLLED_SCCACHE_VARIABLES,
    SETUP_RUST_ACTION,
    SETUP_RUST_SCCACHE_JOBS,
    WORKFLOW_DIR,
    cache_steps,
    declared_paths,
    lane_steps,
)
from fork_fallback import read_placement
from runner_placement_invariants import GITHUB_HOSTED_LABELS
from sccache_compile_step_data import is_compile_step
from workflow_loading import (
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    workflow_job,
)

#: The id each lane gives `setup-rust`, so its report can read the selected
#: backend from `steps.setup-rust.outputs.cache-backend`.
SETUP_RUST_ID = "setup-rust"

#: A full commit pin, the only form a shared action may take here.
FULL_SHA = re.compile(r"^[0-9a-f]{40}$")

#: A `run:` command that starts or resets the server, which `setup-rust` does
#: itself now. A script that did it again would bind whatever backend the
#: script's own environment named.
SERVER_COMMAND = re.compile(r"\bsccache\s+--(?:zero-stats|start-server)\b")

#: The retired local action that exported Ubicloud's proxy credentials.
RETIRED_CREDENTIALS_ACTION = "./.github/actions/sccache-gha-credentials"

#: The retired repository switch between the proxy and a local directory.
RETIRED_SWITCH = "NETSUKE_SCCACHE_LOCAL_DIR"


def _env_findings(scope: str, env: object) -> list[str]:
    """Return the retired variables one `env` mapping sets."""
    if not isinstance(env, dict):
        return []
    return [
        f"{scope} sets {name}" for name in HAND_ROLLED_SCCACHE_VARIABLES if name in env
    ]


def _archived_paths(step: dict[str, object]) -> list[str]:
    """Return the paths a cache step archives, or nothing for any other step."""
    if "actions/cache" not in str(step.get("uses", "")):
        return []
    inputs = step.get("with")
    raw = inputs.get("path", "") if isinstance(inputs, dict) else ""
    return [line.strip() for line in str(raw).splitlines() if line.strip()]


def _installs_sccache(step: dict[str, object]) -> bool:
    """Return whether a step installs its own sccache binary."""
    inputs = step.get("with")
    tool = inputs.get("tool", "") if isinstance(inputs, dict) else ""
    return "install-action@" in str(step.get("uses", "")) and "sccache" in str(tool)


def _step_findings(index: int, step: dict[str, object]) -> list[str]:
    """Return the retired pieces one step carries."""
    findings = _env_findings(f"step {index}", step.get("env"))
    script = str(step.get("run", ""))
    if _installs_sccache(step):
        findings.append(f"step {index} installs its own sccache")
    if SERVER_COMMAND.search(script):
        findings.append(f"step {index} starts or zeroes the sccache server")
    if str(step.get("uses", "")) == RETIRED_CREDENTIALS_ACTION:
        findings.append(f"step {index} runs the retired credentials action")
    if any("sccache" in path for path in _archived_paths(step)):
        findings.append(f"step {index} archives a compiler-cache directory")
    if RETIRED_SWITCH in str(step):
        findings.append(f"step {index} reads {RETIRED_SWITCH}")
    return findings


def hand_rolled_findings(workflow_env: object, job: dict[str, object]) -> list[str]:
    """Return every retired compiler-cache piece a job still carries.

    Pure over the parsed data, so the rule runs on fixtures as well as on the
    checked-in workflows.

    Parameters
    ----------
    workflow_env
        The workflow's top-level `env` mapping, or anything else when it
        declares none.
    job
        One parsed job, with its `steps` list.

    Returns
    -------
    list[str]
        One description per finding; empty for a lane that leaves sccache to
        `setup-rust`.

    Examples
    --------
    >>> hand_rolled_findings({}, {"steps": [{"uses": "actions/checkout@x"}]})
    []
    >>> hand_rolled_findings(
    ...     {"RUSTC_WRAPPER": "sccache"}, {"steps": [{"run": "sccache --zero-stats"}]}
    ... )
    ['workflow sets RUSTC_WRAPPER', 'step 0 starts or zeroes the sccache server']
    """
    findings = _env_findings("workflow", workflow_env)
    findings += _env_findings("job", job.get("env"))
    if RETIRED_SWITCH in str(job.get("env", "")):
        findings.append(f"job reads {RETIRED_SWITCH}")
    steps = job.get("steps")
    for index, step in enumerate(steps if isinstance(steps, list) else []):
        findings += _step_findings(index, step)
    return findings


def expected_expect_cache(runs_on: object) -> str:
    """Return the `expect-cache` value a lane's placement calls for.

    Parameters
    ----------
    runs_on
        The job's parsed `runs-on` value.

    Returns
    -------
    str
        ``"any"`` when the lane has a fork arm on a GitHub-hosted runner,
        else ``"ubicloud"``.

    Examples
    --------
    >>> expected_expect_cache(
    ...     "${{ github.event.pull_request.head.repo.fork"
    ...     " && 'ubuntu-latest' || 'ubicloud-standard-4-ubuntu-2404' }}"
    ... )
    'any'
    >>> expected_expect_cache("ubicloud-standard-4-ubuntu-2404")
    'ubicloud'
    """
    placement = read_placement(runs_on)
    if placement is not None and placement.fork in GITHUB_HOSTED_LABELS:
        return "any"
    return "ubicloud"


def _lane(workflow_name: str, job_name: str) -> tuple[dict[str, object], dict]:
    """Return a lane's workflow document and job mapping."""
    workflow = load_workflow(WORKFLOW_DIR / workflow_name)
    return workflow, workflow_job(workflow, job_name)


def _setup_rust(workflow_name: str, job_name: str) -> dict[str, object]:
    """Return a lane's one `setup-rust` step, failing unless there is exactly one."""
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    calls = [s for s in steps if str(s.get("uses", "")).startswith(SETUP_RUST_ACTION)]
    assert len(calls) == 1, (
        f"{workflow_name} {job_name} must call setup-rust exactly once, "
        f"found {len(calls)}"
    )
    return calls[0]


LANES = list(SETUP_RUST_SCCACHE_JOBS)


@pytest.mark.parametrize(("workflow_name", "job_name"), LANES)
def test_setup_rust_owns_the_compiler_cache(workflow_name: str, job_name: str) -> None:
    """One pinned call, sccache on, and an id the report reads."""
    step = _setup_rust(workflow_name, job_name)
    _, _, pin = str(step["uses"]).partition("@")
    assert FULL_SHA.match(pin), (
        f"{workflow_name} {job_name} must pin setup-rust to a full SHA, not {pin!r}"
    )
    inputs = require_mapping(step.get("with"), "Setup Rust inputs")
    assert str(inputs.get("use-sccache", "true")) == "true", (
        f"{workflow_name} {job_name} switches setup-rust's sccache off, so the "
        "lane compiles with no compiler cache at all"
    )
    assert step.get("id") == SETUP_RUST_ID, (
        f"{workflow_name} {job_name} must give setup-rust the id "
        f"{SETUP_RUST_ID!r}, or its report cannot name the backend"
    )


@pytest.mark.parametrize(
    ("workflow_name", "job_name", "expected"),
    [(*lane, value) for lane, value in SETUP_RUST_SCCACHE_JOBS.items()],
)
def test_each_lane_demands_the_backend_its_placement_allows(
    workflow_name: str, job_name: str, expected: str
) -> None:
    """`ubicloud` fails a proxy-less lane loudly; `any` spares a fork's run."""
    _, job = _lane(workflow_name, job_name)
    placed = expected_expect_cache(job.get("runs-on"))
    assert expected == placed, (
        f"{workflow_name} {job_name} is reviewed as expect-cache {expected!r}, "
        f"but its placement calls for {placed!r}"
    )
    inputs = require_mapping(
        _setup_rust(workflow_name, job_name).get("with"), "Setup Rust inputs"
    )
    assert inputs.get("expect-cache") == expected, (
        f"{workflow_name} {job_name} must pass expect-cache: {expected}, "
        f"not {inputs.get('expect-cache')!r}"
    )


@pytest.mark.parametrize(("workflow_name", "job_name"), LANES)
def test_no_lane_keeps_a_retired_piece(workflow_name: str, job_name: str) -> None:
    """A caller's wrapper, directory or switch overrides `setup-rust` silently."""
    workflow, job = _lane(workflow_name, job_name)
    findings = hand_rolled_findings(workflow.get("env"), job)
    assert not findings, (
        f"{workflow_name} {job_name} still hand-rolls the compiler cache: "
        + "; ".join(findings)
    )


def test_the_gate_cache_owns_no_compiler_cache() -> None:
    """The shared Linux gate cache must not bring the local arm back."""
    source = ACTION_DIR / "linux-gate-cache" / "action.yml"
    text = source.read_text(encoding="utf-8")
    assert "sccache-local" not in text, (
        "linux-gate-cache must not take a compiler-cache backend input any more"
    )
    assert RETIRED_SWITCH not in text, (
        f"linux-gate-cache must not read {RETIRED_SWITCH} any more"
    )
    archived = [
        step
        for step in cache_steps(lane_steps(source, None))
        if any("sccache" in path or "SCCACHE" in path for path in declared_paths(step))
    ]
    assert not archived, f"linux-gate-cache archives a compiler cache: {archived!r}"


@pytest.mark.parametrize(("workflow_name", "job_name"), LANES)
def test_statistics_follow_the_last_compile_and_name_the_backend(
    workflow_name: str, job_name: str
) -> None:
    """`setup-rust` zeroes on start, so the report must follow the build.

    The action must come before the first compile, or that compile runs
    without the wrapper; the report must come after the last, run even on
    failure, and name the backend the action chose.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    setup_at = steps.index(_setup_rust(workflow_name, job_name))
    show = named_step(steps, "Show sccache statistics")
    compiles = [index for index, step in enumerate(steps) if is_compile_step(step)]
    assert compiles, f"{workflow_name} {job_name} compiles nothing"
    assert setup_at < min(compiles), (
        f"{workflow_name} {job_name} compiles before setup-rust starts sccache"
    )
    assert max(compiles) < steps.index(show), (
        f"{workflow_name} {job_name} reports its statistics before the last compile"
    )
    assert show.get("if") == "always()", (
        f"{workflow_name} {job_name} must report its statistics on failure too"
    )
    backend = f"steps.{SETUP_RUST_ID}.outputs.cache-backend"
    assert backend in str(show.get("env", {})), (
        f"{workflow_name} {job_name} must report {backend}"
    )


@pytest.mark.parametrize(
    ("job", "expected"),
    [
        pytest.param({"env": {"SCCACHE_DIR": "x"}, "steps": []}, 1, id="job-dir"),
        pytest.param(
            {
                "steps": [
                    {"uses": "taiki-e/install-action@x", "with": {"tool": "sccache@1"}}
                ]
            },
            1,
            id="installer",
        ),
        pytest.param({"steps": [{"run": "sccache --zero-stats"}]}, 1, id="reset"),
        pytest.param(
            {"steps": [{"uses": "./.github/actions/sccache-gha-credentials"}]},
            1,
            id="credentials",
        ),
        pytest.param(
            {
                "steps": [
                    {"uses": "actions/cache/restore@x", "with": {"path": "~/.sccache"}}
                ]
            },
            1,
            id="archive",
        ),
        pytest.param(
            {"env": {"X": "${{ vars.NETSUKE_SCCACHE_LOCAL_DIR }}"}, "steps": []},
            1,
            id="switch",
        ),
        pytest.param(
            {"steps": [{"run": "sccache --show-stats | tee sccache-stats.txt"}]},
            0,
            id="report",
        ),
        pytest.param(
            {
                "steps": [
                    {"uses": "taiki-e/install-action@x", "with": {"tool": "nextest@1"}}
                ]
            },
            0,
            id="other-tool",
        ),
    ],
)
def test_the_retired_piece_reader_is_narrow_as_well_as_sufficient(
    job: dict[str, typ.Any], expected: int
) -> None:
    """Each retired form is caught, and the report and other tools are not.

    The checked-in lanes can only show that the rule passes on them. These
    fixtures show that it would catch each retired form, and that a rule
    matching the bare word `sccache` would have caught the statistics report
    too.
    """
    findings = hand_rolled_findings({}, job)
    assert len(findings) == expected, (
        f"expected {expected} finding(s) for {job!r}, got {findings!r}"
    )
