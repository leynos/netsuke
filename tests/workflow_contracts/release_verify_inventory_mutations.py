"""Mutate the release-inventory contract to show each assertion is live.

Every assertion in the contract test is paired with a deliberate mutation that
the assertion must reject. This module owns that mutation set, so each mutated
shape sits beside the helper that changes it rather than inside the test that
observes the rejection.
"""

import pytest
from release_verify_inventory_support import (
    DOWNLOAD_STEP,
    LIST_RELEASES_STEP,
    LOCAL_ACTION,
    PUBLISH_JOB,
    TRUSTED_ARTIFACT_VALUE,
    VERIFY_JOB,
    VERIFY_STEP,
    load_action_steps,
)
from workflow_loading import (
    job_steps,
    named_step,
    require_list,
    require_mapping,
    step_index_by_key,
    workflow_job,
)

#: Each ``gh release`` invocation a mutation appends to the download step. The
#: second doubles the space between the words, which the shell still reads as
#: one subcommand and a substring check would not.
RELEASE_MUTATIONS = {
    "gh-release-download": 'gh release download "$RELEASE_TAG"',
    "gh-release-spacing": 'gh  release view "$RELEASE_TAG"',
}
#: Commands a mutation appends with the word after ``gh`` moved past a
#: backslash-newline. Bash reads each pair as one command; a scan of the raw
#: lines sees neither a call nor a release subcommand. The ``midword`` spellings
#: continue *inside* the subcommand name, which joins the halves with nothing
#: between them -- a join that inserted a space would still miss these.
CONTINUATION_MUTATIONS = {
    "gh-api-continued-method": 'gh \\\n  api --method POST "repos/o/r/releases"',
    "gh-api-midword-continued": 'gh a\\\npi --method POST "repos/o/r/releases"',
    "gh-release-continued": "gh \\\n  release delete v1",
    "gh-release-midword-continued": "gh re\\\nlease delete v1",
}
#: Commands a mutation appends after an escaped space, which keeps the ``#``
#: inside the argument instead of opening a comment. A scanner that treats the
#: space as a word boundary discards the call that follows the ``;``.
ESCAPED_SPACE_MUTATIONS = {
    "gh-api-escaped-space": 'true foo\\ #; gh api --method POST "repos/o/r/releases"',
    "gh-release-escaped-space": "true foo\\ #; gh release delete v1",
}
#: Each write-implying argument a mutation adds to the listing call, in the
#: spellings that whole-token matching would accept.
LISTING_ARGUMENT_MUTATIONS = {
    "gh-api-field": "-f x=y",
    "gh-api-inline-field": "--field=x=y",
    "gh-api-short-field": "-fx=y",
    "gh-api-input": "--input=body.json",
    "gh-api-method-override": "--method POST",
    "gh-api-short-method": "-X POST",
}


def _append_release_command(steps: list[dict[str, object]], command: str) -> None:
    """Add one ``gh release`` invocation to the download step."""
    download = named_step(steps, DOWNLOAD_STEP)
    download["run"] = f"{download['run']}{command}\n"


def _add_to_listing_call(steps: list[dict[str, object]], argument: str) -> None:
    """Append one argument to the listing step's ``gh api`` call."""
    listing = named_step(steps, LIST_RELEASES_STEP)
    listing["run"] = str(listing["run"]).replace(
        "--paginate --slurp", f"--paginate --slurp {argument}"
    )


def _hide_write_in_substitution(steps: list[dict[str, object]]) -> None:
    """Move the download call into ``$(...)`` and add a write-implying flag.

    The call no longer begins its line and the field flag turns it into a POST,
    so this mutation is detected only if both halves of the scan work.
    """
    download = named_step(steps, DOWNLOAD_STEP)
    download["run"] = str(download["run"]).replace(
        "gh api --method GET", "body=$(gh api --method GET -f x=y", 1
    )


def _add_secret_to_verify_step(steps: list[dict[str, object]]) -> None:
    """Give the tokenless verification step an environment secret."""
    verify = named_step(steps, VERIFY_STEP)
    require_mapping(verify["env"], "verify environment")["UNEXPECTED_INPUT"] = (
        "${{ secrets.CARGO_REGISTRY_TOKEN }}"
    )


def mutate_action(action: dict[str, object], mutation: str) -> None:
    """Change one dimension of an action step's command or environment."""
    steps = load_action_steps(action)
    if mutation in RELEASE_MUTATIONS:
        _append_release_command(steps, RELEASE_MUTATIONS[mutation])
    elif mutation in CONTINUATION_MUTATIONS:
        _append_release_command(steps, CONTINUATION_MUTATIONS[mutation])
    elif mutation in ESCAPED_SPACE_MUTATIONS:
        _append_release_command(steps, ESCAPED_SPACE_MUTATIONS[mutation])
    elif mutation in LISTING_ARGUMENT_MUTATIONS:
        _add_to_listing_call(steps, LISTING_ARGUMENT_MUTATIONS[mutation])
    elif mutation == "gh-api-substitution":
        _hide_write_in_substitution(steps)
    elif mutation == "secret-token":
        _add_secret_to_verify_step(steps)
    else:
        pytest.fail(f"unknown action mutation {mutation!r}")


def _retarget_trusted_record(workflow: dict[str, object]) -> None:
    """Read the trusted record from the download directory instead.

    The download holds bytes GitHub served, so a verifier pointed there would
    compare the release against itself rather than against the publisher's
    independent record.
    """
    steps = job_steps(workflow, VERIFY_JOB)
    local = steps[step_index_by_key(steps, "uses", LOCAL_ACTION)]
    require_mapping(local["with"], "verifier inputs")["trusted-record-file"] = (
        "release-download/trusted.json"
    )


def _artifact_download_step(workflow: dict[str, object]) -> dict[str, object]:
    """Return the job's ``download-artifact`` step."""
    return next(
        step
        for step in job_steps(workflow, VERIFY_JOB)
        if str(step.get("uses", "")).startswith("actions/download-artifact@")
    )


def _download_by_pattern(workflow: dict[str, object]) -> None:
    """Accept any run's retained artefact rather than this run's."""
    with_values = require_mapping(
        _artifact_download_step(workflow)["with"], "artifact inputs"
    )
    del with_values["name"]
    with_values["pattern"] = "declared-assets-*"


def _download_by_literal(workflow: dict[str, object]) -> None:
    """Repeat the artefact name instead of reading the publisher's output."""
    require_mapping(_artifact_download_step(workflow)["with"], "artifact inputs")[
        "name"
    ] = TRUSTED_ARTIFACT_VALUE


def _break_publisher_output(workflow: dict[str, object]) -> None:
    """Give the publisher an artefact name no rerun would ever produce."""
    publisher = workflow_job(workflow, PUBLISH_JOB)
    outputs = require_mapping(publisher.get("outputs"), "publisher outputs")
    outputs["trusted-artifact"] = "declared-assets-publish-release-1"


def _depend_on_release(workflow: dict[str, object]) -> None:
    """Let the verifier run off the weekly rehearsal instead of a publisher."""
    job = workflow_job(workflow, VERIFY_JOB)
    needs = require_list(job.get("needs"), "verify needs")
    job["needs"] = [*needs, "release"]


def _grant_id_token(workflow: dict[str, object]) -> None:
    """Widen the job's permissions by one write scope."""
    job = workflow_job(workflow, VERIFY_JOB)
    permissions = require_mapping(job["permissions"], "verify permissions")
    permissions["id-token"] = "write"


#: The workflow mutations, each paired with the change it makes.
WORKFLOW_MUTATIONS = {
    "id-token": _grant_id_token,
    "release-dependency": _depend_on_release,
    "trusted-in-download": _retarget_trusted_record,
    "artifact-pattern": _download_by_pattern,
    "artifact-literal": _download_by_literal,
    "publisher-output": _break_publisher_output,
}


def mutate_workflow(workflow: dict[str, object], mutation: str) -> None:
    """Change one dimension of the verification job's wiring."""
    apply = WORKFLOW_MUTATIONS.get(mutation)
    if apply is None:
        pytest.fail(f"unknown workflow mutation {mutation!r}")
    apply(workflow)
