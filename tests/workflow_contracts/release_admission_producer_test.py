"""Hold the wiring between the downstream canaries, the producer, and admission.

Issue #598 supplies the producer the release-admission gate reads. These checks
keep the three links of that chain in place:

- each downstream canary job checks Netsuke out sparsely at the workspace root,
  so the shared action resolves by its own path and the downstream checkout
  inherits no Netsuke Cargo or rustup configuration;
- the producer job runs this repository's own script whatever the canaries
  concluded, and publishes the digest of the manifest it uploads;
- the admission job needs the producer, tolerates no missing artefact, and
  hands the gate that digest beside the interim block's ``false`` literal.

The canary names are spelled out here, so a matrix row added without a
required result, or a required result without a row, fails the comparison.

Run via ``make test-workflow-contracts``.
"""

import importlib.util
import sys
import typing as typ

import pytest
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    read_workflow_document,
    require_list,
    require_mapping,
    workflow_job,
)

if typ.TYPE_CHECKING:
    import types

PRODUCER_JOB = "release-admission-evidence"
ADMISSION_JOB = "release-admission-canaries"
CANARY_JOBS = ("downstream-canaries", "downstream-canaries-windows")
#: Every canary row, Linux then Windows, as the verifier must require them.
LINUX_CANARIES = (
    "repovec-appliance",
    "mxd-postgres",
    "mxd-sqlite",
    "mxd-wireframe-only",
    "ortho-config",
)
WINDOWS_CANARIES = ("ortho-config-windows",)
#: The sparse checkout every canary job makes: the action and its scripts only.
CANARY_CHECKOUT = {
    "repository": "leynos/netsuke",
    "ref": "${{ job.workflow_sha }}",
    "sparse-checkout": "/.github/actions/\n/scripts/\n",
    "sparse-checkout-cone-mode": False,
    "persist-credentials": False,
}
CANARY_ACTION = "./.github/actions/downstream-canary"
PRODUCER_SCRIPT = "gate/.github/scripts/write_release_admission_evidence.py"
DIGEST_OUTPUT = "${{ needs.release-admission-evidence.outputs.evidence_sha256 }}"
DIGEST_STEP_OUTPUT = "${{ steps.write_evidence.outputs.evidence-sha256 }}"
PRODUCER_GUARD = "!cancelled() && needs.metadata.result == 'success'"
ADMISSION_GUARD = (
    "!cancelled() && needs.metadata.result == 'success' && "
    "needs.release-admission-evidence.result == 'success'"
)
RESOLVER_SCRIPT = "gate/.github/scripts/resolve_release_candidate.py"
#: The candidate repository: the one whose refs the resolver read.
CANDIDATE_REPOSITORY = "${{ needs.release-candidate.outputs.repository }}"
#: The release-candidate job's resolver checkout, sparse to the scripts.
RESOLVER_CHECKOUT = {
    "repository": "leynos/netsuke",
    "ref": "${{ job.workflow_sha }}",
    "path": "gate",
    "sparse-checkout": "/.github/scripts/\n",
    "sparse-checkout-cone-mode": False,
    "persist-credentials": False,
}
#: The producer's checkout: this repository at the workflow's own commit.
VERIFIER_CHECKOUT = {
    "persist-credentials": False,
    "repository": "leynos/netsuke",
    "ref": "${{ job.workflow_sha }}",
    "path": "gate",
}


def _release() -> dict[str, object]:
    """Return the parsed release workflow."""
    return require_mapping(
        read_workflow_document(RELEASE_WORKFLOW_PATH), "the release workflow"
    )


def _step(job: str, name: str) -> dict[str, object]:
    """Return the one step of *job* with *name*."""
    matches = [step for step in job_steps(_release(), job) if step.get("name") == name]
    assert len(matches) == 1, f"{job} must have exactly one step named {name!r}"
    return matches[0]


def _downstream_module() -> types.ModuleType:
    """Load the verifier's canary list from its own file."""
    path = REPO_ROOT / ".github" / "scripts" / "_release_admission" / "downstream.py"
    spec = importlib.util.spec_from_file_location("_canary_names", path)
    assert spec is not None, f"cannot load {path}"
    assert spec.loader is not None, f"cannot load {path}"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _matrix_canaries(job: str) -> tuple[str, ...]:
    """Return the canary names one job's matrix declares, in order."""
    strategy = require_mapping(workflow_job(_release(), job).get("strategy"), job)
    matrix = require_mapping(strategy.get("matrix"), f"{job} matrix")
    rows = require_list(matrix.get("include"), f"{job} matrix rows")
    return tuple(str(require_mapping(row, job).get("canary")) for row in rows)


def test_matrix_rows_are_the_required_downstream_results() -> None:
    """Every matrix row is a required result, and nothing else is.

    Scenario: a canary row is added to, or dropped from, the workflow.
    Invariant: the verifier's list moves with it, so no row runs unrequired
    and no required result waits on a row that does not exist.
    """
    downstream = _downstream_module()
    assert _matrix_canaries("downstream-canaries") == LINUX_CANARIES, (
        "the Linux matrix rows must be the documented Linux canaries"
    )
    assert _matrix_canaries("downstream-canaries-windows") == WINDOWS_CANARIES, (
        "the Windows matrix rows must be the documented Windows canaries"
    )
    assert downstream.CANARIES == LINUX_CANARIES + WINDOWS_CANARIES, (
        "the verifier must require exactly the matrix rows"
    )
    assert frozenset(WINDOWS_CANARIES) == downstream.WINDOWS_CANARIES, (
        "the producer must read Windows rows from the Windows job's result"
    )


@pytest.mark.parametrize("job", CANARY_JOBS)
def test_canary_jobs_check_out_only_the_action_and_its_scripts(job: str) -> None:
    """The checkout holds no root file, and the action resolves locally.

    Scenario: the canary checkout is widened, or moved into a subdirectory.
    Invariant: a root ``.cargo/config.toml`` or ``rust-toolchain.toml`` would
    reach the downstream build, and a subdirectory path would hide the action
    from the shared closure check.
    """
    steps = job_steps(_release(), job)
    checkout = require_mapping(steps[0].get("with"), f"{job} checkout inputs")
    assert str(steps[0].get("uses", "")).startswith("actions/checkout@"), (
        f"{job} must begin by checking Netsuke out"
    )
    assert checkout == CANARY_CHECKOUT, (
        f"{job} must check out only the action and its scripts: {checkout!r}"
    )
    assert steps[1].get("uses") == CANARY_ACTION, (
        f"{job} must run the canary action by its workspace-relative path"
    )


def test_producer_runs_after_every_canary_whatever_they_concluded() -> None:
    """A failed canary reaches admission as a declared failure, not a skip.

    Scenario: one canary fails. Invariant: the producer still runs and records
    it; only a cancelled run or a failed metadata job stops it.
    """
    job = workflow_job(_release(), PRODUCER_JOB)
    assert job.get("needs") == ["metadata", *CANARY_JOBS], (
        "the producer must wait for metadata and both canary jobs"
    )
    assert job.get("if") == PRODUCER_GUARD, (
        "the producer must run whatever the canaries concluded"
    )
    assert job.get("permissions") == {"contents": "read"}, (
        "the producer must hold no write scope"
    )


def test_producer_runs_this_repositorys_script_and_publishes_its_digest() -> None:
    """The producer's code and output come from this workflow's own commit.

    Scenario: a reusable-workflow caller supplies its own checkout.
    Invariant: the producer script is checked out at ``job.workflow_sha``, and
    the digest the gate trusts is this job's output of the step that wrote it.
    """
    job = workflow_job(_release(), PRODUCER_JOB)
    checkout = require_mapping(job_steps(_release(), PRODUCER_JOB)[0].get("with"), "")
    assert checkout == VERIFIER_CHECKOUT, (
        f"the producer must run this repository's script: {checkout!r}"
    )
    write = _step(PRODUCER_JOB, "Write the release-admission evidence")
    assert write.get("id") == "write_evidence", "the digest output names this id"
    assert PRODUCER_SCRIPT in str(write.get("run")), (
        "the producer must run the verifier checkout's script"
    )
    assert job.get("outputs") == {"evidence_sha256": DIGEST_STEP_OUTPUT}, (
        "the job must publish the digest of the manifest it wrote"
    )
    upload = require_mapping(
        _step(PRODUCER_JOB, "Upload the release-admission evidence").get("with"), ""
    )
    assert upload.get("name") == "release-admission-evidence", (
        "the upload must use the name the admission job downloads"
    )
    assert upload.get("if-no-files-found") == "error", (
        "a missing manifest must fail the producer"
    )


def test_admission_waits_for_the_producer_and_trusts_only_its_digest() -> None:
    """Admission needs the producer, and attribution is derived, not declared.

    Scenario: the producer is removed, its artefact is missing, or the interim
    block is lifted early. Invariant: the job graph breaks, the download fails,
    and the gate receives the producer's digest beside the block's literal.
    """
    job = workflow_job(_release(), ADMISSION_JOB)
    assert job.get("needs") == ["metadata", PRODUCER_JOB], (
        "admission must wait for the evidence producer"
    )
    assert job.get("if") == ADMISSION_GUARD, (
        "admission must run once the producer succeeded, even when skipped "
        "canaries would make GitHub's implicit success() skip it"
    )
    download = _step(ADMISSION_JOB, "Download the release-admission evidence")
    assert "continue-on-error" not in download, (
        "a missing producer must break the graph, not reach the gate as absence"
    )
    gate = _step(ADMISSION_JOB, "Require release-admission evidence")
    environment = require_mapping(gate.get("env"), "the gate environment")
    assert environment.get("NETSUKE_RELEASE_ADMISSION_EVIDENCE_SHA256") == (
        DIGEST_OUTPUT
    ), "the gate must receive the producer job's own digest output"
    assert environment.get("NETSUKE_RELEASE_ADMISSION_PRODUCER_TRUSTED") == ("false"), (
        "the interim block must stay until both live runs are demonstrated"
    )


def test_release_candidate_runs_this_repositorys_resolver() -> None:
    """The resolver comes from this workflow's commit; the refs, from the caller.

    Scenario: a reusable-workflow caller whose tree lacks the resolver, or
    holds a different one. Invariant: the job checks the resolver out at
    ``job.workflow_sha`` and runs that copy, from the caller's checkout.
    """
    steps = job_steps(_release(), "release-candidate")
    caller = require_mapping(steps[0].get("with"), "caller checkout inputs")
    assert caller.get("fetch-depth") == 0, "the caller's tags must be present"
    assert "repository" not in caller, "the first checkout must be the caller's"
    resolver = require_mapping(steps[1].get("with"), "resolver checkout inputs")
    assert resolver == RESOLVER_CHECKOUT, (
        f"the resolver must come from this workflow's commit: {resolver!r}"
    )
    resolve = _step("release-candidate", "Resolve the release candidate")
    assert RESOLVER_SCRIPT in str(resolve.get("run")), (
        "the job must run the pinned checkout's resolver"
    )
    assert "--no-config" in str(resolve.get("run")), (
        "the caller's uv configuration must not apply"
    )


@pytest.mark.parametrize("job", CANARY_JOBS)
def test_canaries_fetch_the_candidate_from_the_resolvers_repository(job: str) -> None:
    """The installer fetches from the repository the candidate was resolved in.

    Scenario: a reusable-workflow caller resolves a commit that only its own
    repository holds. Invariant: the release-candidate job publishes the
    repository its checkout came from, and each canary passes that output on,
    so the installer never fetches a fixed upstream.
    """
    outputs = require_mapping(
        workflow_job(_release(), "release-candidate").get("outputs"), "outputs"
    )
    assert outputs.get("repository") == "${{ github.repository }}", (
        "the resolver reads the caller's checkout, so that is the repository"
    )
    canary = require_mapping(_step(job, "Run the downstream canary").get("with"), "")
    assert canary.get("netsuke-repository") == CANDIDATE_REPOSITORY, (
        f"{job} must fetch the candidate from the resolver's repository"
    )
