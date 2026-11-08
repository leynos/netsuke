"""Load, tokenize, and mutate the release-inventory verification contract.

The contract test needs the action and the job twice: once to assert their
current shape, and once per deliberate mutation, to show the assertion notices
the change. This module holds the loading, the shell tokenizers, and that
mutation set, so the test module can state the contract while each mutation
stays beside the shape it changes.
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
    step_index_by_key,
    workflow_job,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

VERIFY_JOB = "verify-release-inventory"
ACTION_PATH: Path = (
    REPO_ROOT / ".github" / "actions" / "verify-release-inventory" / "action.yml"
)
LOCAL_ACTION = "./.github/actions/verify-release-inventory"
CHECKOUT_ACTION = "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"
SETUP_UV_ACTION = "astral-sh/setup-uv@1c37ad07a6a961277cf70c0d37d6f313000f5884"
ARTIFACT_ACTION = "actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333"

#: The download must name the publisher's recorded output rather than repeat
#: the value. A "Re-run failed jobs" reuses a successful publisher without
#: running it again, so this job's own attempt number would name an artefact
#: the publisher never uploaded.
TRUSTED_ARTIFACT_OUTPUT = "${{ needs.publish-release.outputs.trusted-artifact }}"
#: The value that output must carry: the name the upload action retains its
#: declaration and digest record under, with the publishing job substituted
#: for `github.job`.
TRUSTED_ARTIFACT_VALUE = "declared-assets-publish-release-${{ github.run_attempt }}"
PUBLISH_JOB = "publish-release"
UPLOAD_ACTION_PATH: Path = (
    REPO_ROOT / ".github" / "actions" / "upload-declared-assets" / "action.yml"
)
UPLOAD_RETAIN_STEP = "Retain the declared asset record"

VALIDATE_STEP = "Validate inputs"
LIST_RELEASES_STEP = "List draft releases"
LIST_ASSETS_STEP = "List draft assets"
DOWNLOAD_STEP = "Download draft assets"
VERIFY_STEP = "Verify inventory"
STEP_NAMES = (
    VALIDATE_STEP,
    LIST_RELEASES_STEP,
    LIST_ASSETS_STEP,
    DOWNLOAD_STEP,
    VERIFY_STEP,
)
NETWORK_STEPS = (LIST_RELEASES_STEP, LIST_ASSETS_STEP, DOWNLOAD_STEP)

EXPECTED_INPUTS = {
    "tag": True,
    "repository": True,
    "trusted-directory": True,
    "declaration-file": True,
    "trusted-record-file": True,
    "download-directory": True,
    "output-path": True,
    "github-token": True,
}
EXPECTED_OUTPUTS = {
    "verified-record-path": "${{ steps.verify.outputs.verified-record-path }}"
}
EXPECTED_ENVIRONMENTS = {
    VALIDATE_STEP: {
        "RELEASE_TAG": "${{ inputs.tag }}",
        "TRUSTED_DIRECTORY": "${{ inputs['trusted-directory'] }}",
        "DECLARATION_FILE": "${{ inputs['declaration-file'] }}",
        "TRUSTED_RECORD_FILE": "${{ inputs['trusted-record-file'] }}",
        "DOWNLOAD_DIRECTORY": "${{ inputs['download-directory'] }}",
        "OUTPUT_PATH": "${{ inputs['output-path'] }}",
    },
    LIST_RELEASES_STEP: {
        "RELEASE_TAG": "${{ inputs.tag }}",
        "RELEASE_REPOSITORY": "${{ inputs.repository }}",
        "GH_TOKEN": "${{ inputs['github-token'] }}",
        "INVENTORY_WORK_DIR": "${{ runner.temp }}/release-inventory",
    },
    LIST_ASSETS_STEP: {
        "RELEASE_REPOSITORY": "${{ inputs.repository }}",
        "GH_TOKEN": "${{ inputs['github-token'] }}",
        "INVENTORY_WORK_DIR": "${{ runner.temp }}/release-inventory",
    },
    DOWNLOAD_STEP: {
        "RELEASE_REPOSITORY": "${{ inputs.repository }}",
        "GH_TOKEN": "${{ inputs['github-token'] }}",
        "INVENTORY_WORK_DIR": "${{ runner.temp }}/release-inventory",
        "DOWNLOAD_DIRECTORY": "${{ inputs['download-directory'] }}",
    },
    # Deliberately tokenless: every byte the verifier reads is already on disk.
    VERIFY_STEP: {
        "DOWNLOAD_DIRECTORY": "${{ inputs['download-directory'] }}",
        "DECLARATION_FILE": "${{ inputs['declaration-file'] }}",
        "TRUSTED_RECORD_FILE": "${{ inputs['trusted-record-file'] }}",
        "OUTPUT_PATH": "${{ inputs['output-path'] }}",
    },
}
#: The verifier invocation, pinned as tokens so neither a swapped path nor an
#: added flag hides inside the shell script.
VERIFY_COMMAND = [
    "uv",
    "run",
    "--no-project",
    "--python",
    "3.14",
    "scripts/verify_release_inventory_cli.py",
    "--directory",
    "$DOWNLOAD_DIRECTORY",
    "--expected",
    "$DECLARATION_FILE",
    "--trusted",
    "$TRUSTED_RECORD_FILE",
    "--output",
    "$OUTPUT_PATH",
]
#: Flags that would turn `gh api` into a write: `-f` and `-F` imply POST, and
#: `--input` reads a body from a file. The action must remain read-only.
FORBIDDEN_GH_FLAGS = frozenset({
    "-f",
    "-F",
    "--field",
    "--raw-field",
    "--input",
})
EXPECTED_JOB_INPUTS = {
    "tag": "${{ needs.metadata.outputs.release_tag }}",
    "repository": "${{ github.repository }}",
    "trusted-directory": "trusted",
    "declaration-file": "trusted/declaration.txt",
    "trusted-record-file": "trusted/trusted.json",
    "download-directory": "release-download",
    "output-path": "verified/verified-digests.json",
    "github-token": "${{ github.token }}",
}


def retained_artifact_name() -> str:
    """Return the name the publisher retains its declared-asset record under.

    Read from the upload action rather than restated, so the verifier's trusted
    artefact and the publisher's retention cannot drift apart without a test
    noticing.

    Returns
    -------
    str
        The upload step's `with.name` expression, still carrying
        ``github.job``.
    """
    action = require_mapping(
        yaml.safe_load(UPLOAD_ACTION_PATH.read_text(encoding="utf-8")),
        "upload-declared-assets action",
    )
    retain = named_step(load_action_steps(action), UPLOAD_RETAIN_STEP)
    return str(require_mapping(retain.get("with"), "retain inputs")["name"])


def load_action() -> dict[str, object]:
    """Read the verification action as a checked mapping."""
    return require_mapping(
        yaml.safe_load(ACTION_PATH.read_text(encoding="utf-8")),
        "verify-release-inventory action",
    )


def load_action_steps(action: dict[str, object]) -> list[dict[str, object]]:
    """Return the action's steps with each step narrowed to a mapping."""
    runs = require_mapping(action.get("runs"), "action runs")
    raw_steps = require_list(runs.get("steps"), "action steps")
    return [require_mapping(step, "action step") for step in raw_steps]


def gh_api_invocations(run: str) -> list[list[str]]:
    """Return the shell tokens of every ``gh api`` invocation in one step.

    A line scanning for ``gh api`` would miss a call whose flags continue onto
    later lines, so the continuation lines are joined first. Tokenizing the
    joined command rather than the whole script keeps the flag checks from
    seeing text that belongs to an unrelated line.

    Returns
    -------
    list[list[str]]
        Shell tokens for each invocation, in the order the step runs them.
    """
    lines = run.splitlines()
    invocations: list[list[str]] = []
    index = 0
    while index < len(lines):
        command = lines[index].strip()
        if command.startswith("gh api"):
            while command.endswith("\\"):
                index += 1
                command = f"{command[:-1]} {lines[index].strip()}"
            invocations.append(shlex.split(command))
        index += 1
    return invocations


def run_tokens(run: str) -> list[str]:
    """Return shell tokens from the step's complete ``uv run`` invocation."""
    lines = run.splitlines()
    start = next(index for index, line in enumerate(lines) if "uv run " in line)
    command = lines[start].strip()
    while command.endswith("\\"):
        start += 1
        command = f"{command[:-1]} {lines[start].strip()}"
    return shlex.split(command)


def reject_secrets(node: object) -> None:
    """Fail if any value in a document references a workflow secret.

    The verifier reads only bytes this job downloaded and inputs its caller
    named; a secret reference would be a new credential in that boundary. The
    walk covers every nested value, so placing one inside an untested step
    does not evade it.
    """
    match node:
        case str() as text:
            assert "secrets." not in text, (
                f"the verification boundary must not reference a secret: {text!r}"
            )
        case dict() as mapping:
            for value in mapping.values():
                reject_secrets(value)
        case list() | tuple() as items:
            for item in items:
                reject_secrets(item)


def mutate_action(action: dict[str, object], mutation: str) -> None:
    """Change one dimension of an action step's command or environment."""
    steps = load_action_steps(action)
    match mutation:
        case "gh-release-download":
            download = named_step(steps, DOWNLOAD_STEP)
            download["run"] = f'{download["run"]}gh release download "$RELEASE_TAG"\n'
        case "gh-api-field":
            listing = named_step(steps, LIST_RELEASES_STEP)
            listing["run"] = str(listing["run"]).replace(
                "--method GET --paginate --slurp", "--method GET --paginate -f x=y"
            )
        case "secret-token":
            verify = named_step(steps, VERIFY_STEP)
            require_mapping(verify["env"], "verify environment")["UNEXPECTED_INPUT"] = (
                "${{ secrets.CARGO_REGISTRY_TOKEN }}"
            )
        case other:
            pytest.fail(f"unknown action mutation {other!r}")


def mutate_workflow(workflow: dict[str, object], mutation: str) -> None:
    """Change one dimension of the verification job's wiring."""
    job = workflow_job(workflow, VERIFY_JOB)
    match mutation:
        case "id-token":
            permissions = require_mapping(job["permissions"], "verify permissions")
            permissions["id-token"] = "write"
        case "release-dependency":
            needs = require_list(job.get("needs"), "verify needs")
            job["needs"] = [*needs, "release"]
        case "trusted-in-download":
            steps = job_steps(workflow, VERIFY_JOB)
            local = steps[step_index_by_key(steps, "uses", LOCAL_ACTION)]
            require_mapping(local["with"], "verifier inputs")["trusted-record-file"] = (
                "release-download/trusted.json"
            )
        case "artifact-pattern":
            download = next(
                step
                for step in job_steps(workflow, VERIFY_JOB)
                if str(step.get("uses", "")).startswith("actions/download-artifact@")
            )
            with_values = require_mapping(download["with"], "artifact inputs")
            del with_values["name"]
            with_values["pattern"] = "declared-assets-*"
        case "artifact-literal":
            download = next(
                step
                for step in job_steps(workflow, VERIFY_JOB)
                if str(step.get("uses", "")).startswith("actions/download-artifact@")
            )
            require_mapping(download["with"], "artifact inputs")["name"] = (
                TRUSTED_ARTIFACT_VALUE
            )
        case "publisher-output":
            publisher = workflow_job(workflow, PUBLISH_JOB)
            outputs = require_mapping(publisher.get("outputs"), "publisher outputs")
            outputs["trusted-artifact"] = "declared-assets-publish-release-1"
        case other:
            pytest.fail(f"unknown workflow mutation {other!r}")
