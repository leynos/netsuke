"""Load, tokenize, and mutate the release-inventory verification contract.

The contract test needs the action and the job twice: once to assert their
current shape, and once per deliberate mutation, to show the assertion notices
the change. This module holds the loading, the shell tokenizers, and that
mutation set, so the test module can state the contract while each mutation
stays beside the shape it changes.
"""

import re
import shlex
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
#: Long flags that would turn `gh api` into a write: `--field` and
#: `--raw-field` imply POST, and `--input` reads a body from a file. `--method`
#: is checked separately, because the call's own leading one is required.
FORBIDDEN_LONG_FLAGS = ("--field", "--raw-field", "--input")
#: The short spellings of the same flags. `-f` and `-F` imply POST and `-X`
#: sets the method. Matching their first two characters covers the joined
#: forms, such as `-fx=y` and `-XPOST`.
FORBIDDEN_SHORT_FLAGS = ("-f", "-F", "-X")
#: A `gh api` call anywhere on a line, including inside a command
#: substitution, which a check anchored at the start of the line would miss.
GH_API_CALL = re.compile(r"\bgh\s+api\b")
#: The `gh release` subcommand, which can write; `gh api` is what a read-only
#: step must use instead.
GH_RELEASE_CALL = re.compile(r"\bgh\s+release\b")
#: A workflow expression naming the secrets context, whether dotted, indexed,
#: or passed whole to `toJSON`.
SECRET_REFERENCE = re.compile(r"\$\{\{[^}]*\bsecrets\b")
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
    seeing text that belongs to an unrelated line. The search starts wherever
    the call sits on its line, so a call inside a command substitution is
    scanned like any other.

    Returns
    -------
    list[list[str]]
        Shell tokens for each invocation, in the order the step runs them.

    Raises
    ------
    AssertionError
        If the script names more ``gh api`` calls than the scan tokenizes. An
        unmodelled shape must fail the contract rather than slip past it.
    """
    lines = run.splitlines()
    # A comment that mentions `gh api` names no call, so it neither counts
    # towards the total nor gets tokenized as though it were one.
    script = [line for line in lines if not line.lstrip().startswith("#")]
    invocations: list[list[str]] = []
    index = 0
    while index < len(script):
        command = script[index].strip()
        call = GH_API_CALL.search(command)
        if call is not None:
            command = command[call.start() :]
            while command.endswith("\\") and index + 1 < len(script):
                index += 1
                command = f"{command[:-1]} {script[index].strip()}"
            invocations.append(shlex.split(command))
        index += 1
    mentioned = len(GH_API_CALL.findall("\n".join(script)))
    if mentioned != len(invocations):
        msg = (
            f"every gh api call must be tokenized, found {mentioned} call(s) "
            f"and {len(invocations)} invocation(s)"
        )
        raise AssertionError(msg)
    return invocations


def forbidden_gh_flags(tokens: list[str]) -> list[str]:
    """Return each argument of one ``gh api`` call that implies a write.

    Matching whole tokens would accept ``--field=x=y`` and ``-fx=y``, so each
    argument is compared by its flag name with any ``=value`` removed. The
    call's own leading ``--method GET`` is the one method override the action
    is allowed to name; any later ``--method`` is a second, unreviewed one.

    Returns
    -------
    list[str]
        The offending arguments, in the order they appear.
    """
    offending: list[str] = []
    # The contract pins `gh api --method GET` as the leading four tokens, so
    # that exact method is the one override the action may name.
    allowed_method = tokens[:4] == ["gh", "api", "--method", "GET"]
    for position, token in enumerate(tokens):
        name = token.split("=", 1)[0]
        if name == "--method":
            if position != 2 or not allowed_method:
                offending.append(token)
        elif name in FORBIDDEN_LONG_FLAGS or name[:2] in FORBIDDEN_SHORT_FLAGS:
            offending.append(token)
    return offending


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
            assert not SECRET_REFERENCE.search(text), (
                f"the verification boundary must not reference a secret: {text!r}"
            )
        case dict() as mapping:
            for value in mapping.values():
                reject_secrets(value)
        case list() | tuple() as items:
            for item in items:
                reject_secrets(item)
