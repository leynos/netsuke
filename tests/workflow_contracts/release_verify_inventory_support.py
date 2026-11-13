"""Load and tokenize the documents the release-inventory contract reads.

The contract test asserts the action's and the job's current shape, and then
re-asserts it once per deliberate mutation to show that the assertion notices
the change. This module holds the documents the contract reads and the values
it pins; how a step's ``run`` text becomes the commands bash would execute, and
how a ``gh api`` call or an expression is recognised inside it, live in
``release_verify_inventory_commands.py`` and
``release_verify_inventory_command_words.py``.

The mutations themselves live in ``release_verify_inventory_mutations.py``,
beside the shape each one changes, so this module stays the reading side of the
contract rather than growing a second responsibility.
"""

import typing as typ

import yaml
from workflow_loading import (
    REPO_ROOT,
    named_step,
    require_list,
    require_mapping,
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
    "verified-record-path": "${{ steps.verify.outputs.verified-record-path }}",
    # The job's telemetry reads these, so each phase's outcome and timing has
    # to be published by the action: the workflow cannot see the boundary
    # between the two listings, the downloads, and the verifier. The listing
    # phase reports the release listing's own outcome when that step fails,
    # because the asset listing is then skipped and says only that it did not
    # run.
    "release-listing-outcome": (
        "${{ steps.list-releases.outcome != 'success'"
        " && steps.list-releases.outcome || steps.list-assets.outcome }}"
    ),
    "release-listing-started-ns": "${{ steps.list-releases.outputs.started_ns }}",
    "release-listing-ended-ns": "${{ steps.list-assets.outputs.ended_ns }}",
    "release-download-outcome": "${{ steps.download.outcome }}",
    "release-download-started-ns": "${{ steps.download.outputs.started_ns }}",
    "release-download-ended-ns": "${{ steps.download.outputs.ended_ns }}",
    "inventory-verification-outcome": "${{ steps.verify.outcome }}",
    "inventory-verification-started-ns": "${{ steps.verify.outputs.started_ns }}",
    "inventory-verification-ended-ns": "${{ steps.verify.outputs.ended_ns }}",
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
    """Read the verification action as a checked mapping.

    Returns
    -------
    dict[str, object]
        The action document: its inputs, outputs and ``runs`` mapping, read
        from disk afresh so one case's mutation cannot reach the next.
    """
    return require_mapping(
        yaml.safe_load(ACTION_PATH.read_text(encoding="utf-8")),
        "verify-release-inventory action",
    )


def load_action_steps(action: dict[str, object]) -> list[dict[str, object]]:
    """Return the action's steps with each step narrowed to a mapping.

    Parameters
    ----------
    action
        The action document :func:`load_action` read, whose ``runs.steps``
        list this call narrows.

    Returns
    -------
    list[dict[str, object]]
        The steps in the order the action runs them, each a mapping whose
        ``name``, ``uses``, ``run``, ``with`` and ``env`` keys the contract
        reads.
    """
    runs = require_mapping(action.get("runs"), "action runs")
    raw_steps = require_list(runs.get("steps"), "action steps")
    return [require_mapping(step, "action step") for step in raw_steps]
