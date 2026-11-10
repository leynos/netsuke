"""Load, tokenize, and mutate the release-finalization contract.

The finalization job is the only stage that changes a release's visibility, so
its contract is asserted twice: against the checked-in action and workflow, and
against one deliberate mutation per property, to show each assertion notices
the change rather than passing because nothing varies. This module holds the
loading, the command tokenizers, and the mutation set so the test module can
state the contract while each mutation stays beside the shape it changes.

Run via ``make test-workflow-contracts``.
"""

import shlex
import typing as typ

import pytest
import yaml
from workflow_loading import (
    REPO_ROOT,
    job_steps,
    named_step,
    require_list,
    require_mapping,
    workflow_job,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

FINALIZE_JOB = "finalize-github-release"
ACTION_PATH: Path = (
    REPO_ROOT / ".github" / "actions" / "finalize-github-release" / "action.yml"
)
LOCAL_ACTION = "./.github/actions/finalize-github-release"
CHECKOUT_ACTION = "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"
SETUP_UV_ACTION = "astral-sh/setup-uv@1c37ad07a6a961277cf70c0d37d6f313000f5884"
ARTIFACT_ACTION = "actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333"

PUBLISH_JOB = "publish-release"
VERIFY_JOB = "verify-release-inventory"
CRATES_JOB = "publish-crates-io"
PACKAGE_JOB = "package-crate"
#: The publisher's output naming the artefact it retained. A "Re-run failed
#: jobs" reuses a successful publisher without running it again, so this job's
#: own attempt number would name an artefact nothing uploaded.
TRUSTED_ARTIFACT_OUTPUT = "${{ needs.publish-release.outputs.trusted-artifact }}"
TRUSTED_ARTIFACT_VALUE = "declared-assets-publish-release-${{ github.run_attempt }}"

VALIDATE_STEP = "Validate inputs"
RESOLVE_STEP = "Resolve the release candidate"
SNAPSHOT_STEP = "Snapshot the release state"
PLAN_STEP = "Plan the release reconciliation"
FINALIZE_STEP = "Finalize the release"
STEP_NAMES = (VALIDATE_STEP, RESOLVE_STEP, SNAPSHOT_STEP, PLAN_STEP, FINALIZE_STEP)
#: The only steps that may hold a token: one reads the draft, the other is the
#: single mutating call. The resolver and the planner read local records.
TOKEN_STEPS = (SNAPSHOT_STEP, FINALIZE_STEP)

#: The whole-job ceiling the action's step ceiling must sit beneath. The pinned
#: Cuprum release bounds no call in process, so a timeout is the only thing
#: that stops a hung `gh` from holding the job open.
JOB_TIMEOUT_MINUTES = 30

#: The ceiling on the workflow step that invokes the composite action. It sits
#: there rather than on the action's own steps because the runner does not
#: support `timeout-minutes` inside a composite action (actions/runner#1979),
#: which is why the action bounds each command with GNU `timeout` instead.
FINALIZE_STEP_TIMEOUT_MINUTES = 28

#: The longest per-command bound the action may give one adapter invocation.
#: Four of them, at most, must fit under :data:`FINALIZE_STEP_TIMEOUT_MINUTES`;
#: the contract sums them rather than trusting the individual caps to imply it.
MAX_COMMAND_BOUND_SECONDS = 600

EXPECTED_INPUTS = {
    "tag": True,
    "commit": True,
    "repository": True,
    "identity-path": True,
    "digest-record-path": True,
    "work-directory": True,
    "github-token": True,
}
EXPECTED_OUTPUTS = {
    "outcome": "${{ steps.finalize.outputs.outcome }}",
    "prerelease": "${{ steps.finalize.outputs.prerelease }}",
    "plan-path": "${{ steps.plan.outputs.plan-path }}",
}
EXPECTED_ENVIRONMENTS = {
    VALIDATE_STEP: {
        "RELEASE_TAG": "${{ inputs.tag }}",
        "RELEASE_COMMIT": "${{ inputs.commit }}",
        "RELEASE_REPOSITORY": "${{ inputs.repository }}",
        "IDENTITY_PATH": "${{ inputs['identity-path'] }}",
        "DIGEST_RECORD_PATH": "${{ inputs['digest-record-path'] }}",
        "WORK_DIRECTORY": "${{ inputs['work-directory'] }}",
        "HAS_GITHUB_TOKEN": "${{ inputs['github-token'] != '' }}",
    },
    RESOLVE_STEP: {
        "RELEASE_TAG": "${{ inputs.tag }}",
        "RELEASE_COMMIT": "${{ inputs.commit }}",
        "IDENTITY_PATH": "${{ inputs['identity-path'] }}",
        "WORK_DIRECTORY": "${{ inputs['work-directory'] }}",
    },
    SNAPSHOT_STEP: {
        "RELEASE_TAG": "${{ inputs.tag }}",
        "RELEASE_REPOSITORY": "${{ inputs.repository }}",
        "WORK_DIRECTORY": "${{ inputs['work-directory'] }}",
        "GH_TOKEN": "${{ inputs['github-token'] }}",
    },
    PLAN_STEP: {
        "DIGEST_RECORD_PATH": "${{ inputs['digest-record-path'] }}",
        "WORK_DIRECTORY": "${{ inputs['work-directory'] }}",
    },
    FINALIZE_STEP: {
        "RELEASE_TAG": "${{ inputs.tag }}",
        "RELEASE_COMMIT": "${{ inputs.commit }}",
        "RELEASE_REPOSITORY": "${{ inputs.repository }}",
        "WORK_DIRECTORY": "${{ inputs['work-directory'] }}",
        "GH_TOKEN": "${{ inputs['github-token'] }}",
    },
}

#: The only edit the finalizer may make, pinned as tokens so a swapped flag
#: cannot hide inside the shell script.
FINALIZE_COMMAND = [
    "uv",
    "run",
    "--no-project",
    "--python",
    "3.14",
    "scripts/finalize_github_release.py",
    "publish",
    "--tag",
    "$RELEASE_TAG",
    "--commit",
    "$RELEASE_COMMIT",
    "--repository",
    "$RELEASE_REPOSITORY",
    "--candidate-path",
    "$WORK_DIRECTORY/candidate.json",
    "--plan-path",
    "$WORK_DIRECTORY/plan.json",
    "--result-path",
    "$WORK_DIRECTORY/result.json",
    "--github-output",
    "$GITHUB_OUTPUT",
]
#: A finalizer that calls `gh` itself would put an unbounded, unparsed process
#: in the step the adapter exists to keep shell-free.
FORBIDDEN_IN_RUN = ("gh release", "gh api", "gh pr")

#: The plan step deletes any earlier plan before planning. Without it a planner
#: run that wrote nothing would leave the previous attempt's plan to be read as
#: this run's answer.
PLAN_RESET = 'rm -f "$WORK_DIRECTORY/plan.json"'
#: The plan step refuses to report a plan path the run did not write. Its exit
#: statuses cannot say a plan exists: `argparse` usage errors and `uv`
#: resolution failures also exit 2, and an uncaught exception exits 1.
PLAN_PRESENCE_GUARD = 'if [[ ! -f "$WORK_DIRECTORY/plan.json" ]]; then'
EXPECTED_JOB_INPUTS = {
    "tag": "${{ needs.metadata.outputs.release_tag }}",
    "commit": "${{ needs.metadata.outputs.candidate_commit }}",
    "repository": "${{ github.repository }}",
    "identity-path": "release-crate-package/artifacts/package/identity.json",
    "digest-record-path": "trusted/trusted.json",
    "work-directory": "${{ runner.temp }}/release-finalize",
    "github-token": "${{ github.token }}",
}
#: Every producer whose success the finalization's evidence rests on.
EXPECTED_NEEDS = (
    "metadata",
    PACKAGE_JOB,
    CRATES_JOB,
    PUBLISH_JOB,
    VERIFY_JOB,
)
EXPECTED_PERMISSIONS = {"contents": "write"}


def load_action() -> dict[str, object]:
    """Read the finalization action as a checked mapping."""
    return require_mapping(
        yaml.safe_load(ACTION_PATH.read_text(encoding="utf-8")),
        "finalize-github-release action",
    )


def load_action_steps(action: dict[str, object]) -> list[dict[str, object]]:
    """Return the action's steps with each step narrowed to a mapping."""
    runs = require_mapping(action.get("runs"), "action runs")
    raw_steps = require_list(runs.get("steps"), "action steps")
    return [require_mapping(step, "action step") for step in raw_steps]


def run_tokens(run: str) -> list[str]:
    """Return shell tokens from the step's first ``uv run`` invocation."""
    lines = run.splitlines()
    start = next(index for index, line in enumerate(lines) if "uv run " in line)
    command = lines[start].strip()
    while command.endswith("\\"):
        start += 1
        command = f"{command[:-1]} {lines[start].strip()}"
    return shlex.split(command)


def mutate_action(action: dict[str, object], mutation: str) -> None:
    """Change one dimension of an action step's command or environment."""
    steps = load_action_steps(action)
    match mutation:
        case "token-on-validate":
            validate = named_step(steps, VALIDATE_STEP)
            # The shape this action actually had: the credential handed to a
            # step that only needed to know whether one had been supplied.
            require_mapping(validate["env"], "validate environment")["GITHUB_TOKEN"] = (
                EXPECTED_ENVIRONMENTS[SNAPSHOT_STEP]["GH_TOKEN"]
            )
        case "token-on-plan":
            plan = named_step(steps, PLAN_STEP)
            # Reuse the expression the snapshot step legitimately holds, so the
            # mutation differs from the contract in exactly one place.
            require_mapping(plan["env"], "plan environment")["GH_TOKEN"] = (
                EXPECTED_ENVIRONMENTS[SNAPSHOT_STEP]["GH_TOKEN"]
            )
        case "unbounded-command":
            finalize = named_step(steps, FINALIZE_STEP)
            # The action's only available bound, removed: the adapter now runs
            # with nothing to stop a hung `gh` call.
            run = str(finalize["run"])
            finalize["run"] = "\n".join(
                line
                for line in run.splitlines()
                if not line.strip().startswith("timeout --kill-after=")
            )
        case "aggregate-bound-exceeded":
            resolve = named_step(steps, RESOLVE_STEP)
            # Every bound stays inside its own cap; together they outlast the
            # step that contains them.
            resolve["run"] = str(resolve["run"]).replace(
                "timeout --kill-after=30s 300s",
                f"timeout --kill-after=30s {MAX_COMMAND_BOUND_SECONDS}s",
            )
        case "stale-plan-kept" | "missing-plan-guard":
            _mutate_plan_step(steps, mutation)
        case "gh-in-run":
            finalize = named_step(steps, FINALIZE_STEP)
            finalize["run"] = f'{finalize["run"]}gh release delete "$RELEASE_TAG"\n'
        case "wrong-output":
            outputs = require_mapping(action["outputs"], "action outputs")
            require_mapping(outputs["outcome"], "outcome output")["value"] = (
                "${{ steps.resolve.outputs.outcome }}"
            )
        case other:
            pytest.fail(f"unknown action mutation {other!r}")


def _mutate_plan_step(steps: list[dict[str, object]], mutation: str) -> None:
    """Break one of the plan step's two guards against a plan-less run."""
    plan = named_step(steps, PLAN_STEP)
    match mutation:
        case "stale-plan-kept":
            # The earlier attempt's plan survives, so a planner run that wrote
            # nothing would leave it to be read as this run's answer.
            plan["run"] = "\n".join(
                line
                for line in str(plan["run"]).splitlines()
                if line.strip() != PLAN_RESET
            )
        case "missing-plan-guard":
            # The guard stays syntactically present but never fires, which is
            # the shape of a check that was written and then disabled.
            plan["run"] = str(plan["run"]).replace(
                PLAN_PRESENCE_GUARD, "if false; then"
            )
        case other:
            pytest.fail(f"unknown plan-step mutation {other!r}")


def mutate_workflow(workflow: dict[str, object], mutation: str) -> None:
    """Change one dimension of the finalization job's wiring."""
    job = workflow_job(workflow, FINALIZE_JOB)
    match mutation:
        case "id-token":
            permissions = require_mapping(job["permissions"], "finalize permissions")
            permissions["id-token"] = "write"
        case "release-dependency":
            needs = require_list(job.get("needs"), "finalize needs")
            job["needs"] = [*needs, "release"]
        case "missing-verifier":
            needs = require_list(job.get("needs"), "finalize needs")
            job["needs"] = [name for name in needs if name != VERIFY_JOB]
        case "artifact-pattern":
            download = _trusted_download(workflow)
            with_values = require_mapping(download["with"], "artifact inputs")
            del with_values["name"]
            with_values["pattern"] = "declared-assets-*"
        case "trusted-literal":
            download = _trusted_download(workflow)
            require_mapping(download["with"], "artifact inputs")["name"] = (
                TRUSTED_ARTIFACT_VALUE
            )
        case "missing-step-timeout":
            local = _finalize_step(workflow)
            del local["timeout-minutes"]
        case "verifier-record":
            local = _finalize_step(workflow)
            require_mapping(local["with"], "finalizer inputs")["digest-record-path"] = (
                "verified/verified-digests.json"
            )
        case other:
            pytest.fail(f"unknown workflow mutation {other!r}")


def _trusted_download(workflow: dict[str, object]) -> dict[str, object]:
    """Return the job step that downloads the trusted digest record."""
    return next(
        step
        for step in job_steps(workflow, FINALIZE_JOB)
        if str(step.get("uses", "")).startswith("actions/download-artifact@")
        and require_mapping(step.get("with"), "artifact inputs").get("path")
        == "trusted"
    )


def _finalize_step(workflow: dict[str, object]) -> dict[str, object]:
    """Return the job step that invokes the repository's finalization action."""
    return next(
        step
        for step in job_steps(workflow, FINALIZE_JOB)
        if step.get("uses") == LOCAL_ACTION
    )
