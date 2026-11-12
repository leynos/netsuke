"""Load and tokenize the documents the release-inventory contract reads.

The contract test asserts the action's and the job's current shape, and then
re-asserts it once per deliberate mutation to show that the assertion notices
the change. This module holds the shared loading and the shell tokenizers: how
a step's ``run`` text becomes the commands bash would execute, and how an
expression or a ``gh api`` call is recognised inside it.

The mutations themselves live in ``release_verify_inventory_mutations.py``,
beside the shape each one changes, so this module stays the reading side of the
contract rather than growing a second responsibility.
"""

import re
import shlex
import typing as typ

import release_verify_inventory_shell as shell
import yaml
from workflow_loading import (
    REPO_ROOT,
    named_step,
    require_list,
    require_mapping,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc
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
#: The word naming the secrets context inside an expression, whether dotted,
#: indexed, or passed whole to `toJSON`.
SECRET_WORD = re.compile(r"\bsecrets\b")
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


def script_lines(run: str) -> list[str]:
    r"""Return a step's shell commands as bash reads them, one per line.

    A backslash-newline is deleted before bash parses, so ``gh a\\`` on one
    line and ``pi`` on the next are one ``gh api`` call -- and a scan of the raw
    lines sees neither. The quoting and escape rules that make the deletion
    conditional live in ``release_verify_inventory_shell``.

    Returns
    -------
    list[str]
        The joined commands, in the order the step runs them.
    """
    return shell.logical_lines(run)


def gh_api_invocations(run: str) -> list[list[str]]:
    """Return the shell tokens of every ``gh api`` invocation in one step.

    Tokenizing one joined command rather than the whole script keeps the flag
    checks from seeing text that belongs to an unrelated line. The search
    starts wherever the call sits on its line, so a call inside a command
    substitution is scanned like any other.

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
    script = script_lines(run)
    invocations = [
        shlex.split(line[call.start() :])
        for line in script
        if (call := GH_API_CALL.search(line)) is not None
    ]
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
    """Return shell tokens from the step's complete ``uv run`` invocation.

    The invocation is read as a logical line, so a continuation inside it is
    already joined the way bash joins it and the token list cannot be cut short
    at a backslash.

    Returns
    -------
    list[str]
        The invocation's shell tokens, in the order it passes them.
    """
    command = next(line for line in script_lines(run) if "uv run " in line)
    return shlex.split(command)


def expression_bodies(text: str) -> cabc.Iterator[str]:
    """Yield the body of every ``${{ ... }}`` expression in ``text``.

    Looking for the first closing brace stops inside a quoted format string:
    ``format('Hello {0}', secrets.TOKEN)`` closes at the ``0`` and the secret
    that follows is never examined. GitHub expressions quote with single
    quotes only, so a single-quoted run is skipped whole and a brace inside it
    cannot end the expression early.

    Yields
    ------
    str
        Each expression's text, without its ``${{`` and ``}}`` delimiters.
    """
    start = 0
    while (opening := text.find("${{", start)) != -1:
        quoted = False
        for index in range(opening + 3, len(text)):
            char = text[index]
            if quoted:
                quoted = char != "'"
            elif char == "'":
                quoted = True
            elif text.startswith("}}", index):
                yield text[opening + 3 : index]
                start = index + 2
                break
        else:
            return


def references_secret(text: str) -> bool:
    """Return whether any expression in ``text`` names the secrets context."""
    return any(SECRET_WORD.search(body) for body in expression_bodies(text))


def reject_secrets(node: object) -> None:
    """Fail if any value in a document references a workflow secret.

    The verifier reads only bytes this job downloaded and inputs its caller
    named; a secret reference would be a new credential in that boundary. The
    walk covers every nested value, so placing one inside an untested step
    does not evade it.
    """
    match node:
        case str() as text:
            assert not references_secret(text), (
                f"the verification boundary must not reference a secret: {text!r}"
            )
        case dict() as mapping:
            for value in mapping.values():
                reject_secrets(value)
        case list() | tuple() as items:
            for item in items:
                reject_secrets(item)
