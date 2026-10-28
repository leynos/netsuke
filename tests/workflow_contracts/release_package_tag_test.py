"""Keep the packaging job from feeding a merge ref to the tag validator.

`package-crate` runs in both release modes. On a tag push `github.ref_name` is
the release tag, and the identity it records must name that tag. On a
pull-request dry run the same expression resolves to a *merge ref* such as
`903/merge`, which is not a tag at all: the packaging CLI refused it with
``tag '903/merge' is not a release tag``, so the rehearsal failed at identity
resolution before cargo ever ran.

The repair is not to loosen the validator. `release_candidate` exists to refuse
a tag that does not point at the commit the bytes were built from, and the
merge commit is exactly such a case — recording ``v0.1.0-beta4`` for it would
be the near-match that module is built to reject. So the mode is carried
explicitly instead: the job passes the same ``should_publish``-derived flag
`upload-release-assets` already receives, and the action selects a tag *or* a
rehearsal before invoking the CLI.

These checks hold that wiring, because the failure they prevent is silent in
the local suites and only appears in a pull-request run.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

import pytest
import yaml
from workflow_loading import (
    RELEASE_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
    require_list,
    require_mapping,
    workflow_job,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

PACKAGE_JOB = "package-crate"
PACKAGE_STEP = "Package the crate and record its identity"
PUBLISH_JOB = "publish-release"
ACTION_MANIFEST: Path = (
    REPO_ROOT / ".github" / "actions" / "package-crate" / "action.yml"
)

#: The exact mode flag the packaging step must receive. Compared whole: this is
#: the same expression `upload-release-assets` is given, so the two stages
#: cannot disagree about whether the run is publishing.
REHEARSAL_FLAG = "${{ needs.metadata.outputs.should_publish != 'true' }}"


def _packaging_step() -> dict[str, object]:
    """Return the packaging step from the packaging job."""
    steps = job_steps(load_workflow(RELEASE_WORKFLOW_PATH), PACKAGE_JOB)
    return named_step(steps, PACKAGE_STEP)


def _action_manifest() -> dict[str, object]:
    """Return the composite action's parsed manifest."""
    return require_mapping(
        yaml.safe_load(ACTION_MANIFEST.read_text(encoding="utf-8")),
        "package-crate action manifest",
    )


def _action_inputs_and_environment() -> tuple[dict[str, object], dict[str, object]]:
    """Return the manifest inputs and package-step environment together."""
    manifest = _action_manifest()
    inputs = require_mapping(manifest.get("inputs"), "action inputs")
    steps = require_list(
        require_mapping(manifest.get("runs"), "action runs").get("steps"),
        "action steps",
    )
    named_steps = [require_mapping(step, "action step") for step in steps]
    package = require_mapping(
        named_step(named_steps, "Package crate"), "package action step"
    )
    environment = require_mapping(package.get("env"), "package action environment")
    return inputs, environment


def _action_run_body() -> str:
    """Return every ``run`` block the action declares, joined for inspection."""
    runs = require_mapping(_action_manifest().get("runs"), "action runs")
    steps = require_list(runs.get("steps"), "action steps")
    return "\n".join(
        str(require_mapping(step, "action step").get("run", "")) for step in steps
    )


def test_the_packaging_step_receives_the_rehearsal_mode() -> None:
    """Pass the mode flag, so a pull-request run is never read as publishing.

    Without it the step would have no way to distinguish a tag push from a
    rehearsal, and would forward the merge ref as though it were a tag.
    """
    inputs = require_mapping(_packaging_step().get("with"), "packaging step inputs")

    assert inputs.get("dry-run") == REHEARSAL_FLAG, (
        "the packaging step must receive the same publish-mode flag as the "
        f"upload stage, got {inputs.get('dry-run')!r}"
    )


def test_publishing_requires_a_validated_release_tag() -> None:
    """Reject non-tag reusable-workflow calls before exposing a publish tag."""
    workflow = load_workflow(RELEASE_WORKFLOW_PATH)
    metadata = workflow_job(workflow, "metadata")
    outputs = require_mapping(metadata.get("outputs"), "metadata outputs")
    steps = job_steps(workflow, "metadata")
    validation = named_step(steps, "Validate publishing ref")
    environment = require_mapping(validation.get("env"), "ref validation environment")

    assert outputs.get("release_tag") == (
        "${{ steps.validate_publish_ref.outputs.release_tag }}"
    ), "the metadata output must come from the ref-validation step"
    assert validation.get("id") == "validate_publish_ref", (
        "the output reference and validation step identity must stay aligned"
    )
    assert validation.get("if") == (
        "steps.release_modes.outputs['should-publish'] == 'true'"
    ), "only publishing runs need a release ref"
    assert environment.get("RELEASE_REF") == "${{ github.ref }}", (
        "the check must inspect the full ref, not a reusable-workflow input"
    )
    run = str(validation.get("run", ""))
    assert "refs/tags/v*.*.*" in run, "publishing must require a release tag ref"
    assert "printf 'release_tag=%s\\n' \"$release_tag\"" in run, (
        "only the validated tag may be exposed to downstream jobs"
    )

    package_step = named_step(job_steps(workflow, PACKAGE_JOB), PACKAGE_STEP)
    package_inputs = require_mapping(package_step.get("with"), "package inputs")
    assert package_inputs.get("tag") == "${{ needs.metadata.outputs.release_tag }}", (
        "crate identity must use the validated release tag"
    )


def test_only_the_validated_tag_reaches_publishing_steps() -> None:
    """Keep raw refs out of the real GitHub release publication path."""
    workflow = load_workflow(RELEASE_WORKFLOW_PATH)
    staging_steps = job_steps(workflow, "release")
    staging_upload = named_step(staging_steps, "Upload artefacts to release")
    staging_inputs = require_mapping(
        staging_upload.get("with"), "staging upload inputs"
    )
    assert staging_inputs.get("release-tag") == (
        "${{ needs.metadata.outputs.release_tag || github.ref_name }}"
    ), "a rehearsal needs a non-empty tag for the upload action's dry-run plan"
    assert staging_inputs.get("dry-run") is True, (
        "the raw-ref fallback is safe only while the staging upload is a dry run"
    )

    publish_steps = job_steps(workflow, PUBLISH_JOB)
    draft = named_step(publish_steps, "Ensure release exists (draft)")
    draft_env = require_mapping(draft.get("env"), "draft release environment")
    assert draft_env.get("INPUT_TAG") == "${{ needs.metadata.outputs.release_tag }}", (
        "draft creation must use the validated tag"
    )

    upload = named_step(publish_steps, "Upload artefacts to release")
    upload_inputs = require_mapping(upload.get("with"), "publish upload inputs")
    assert upload_inputs.get("release-tag") == (
        "${{ needs.metadata.outputs.release_tag }}"
    ), "a real asset upload must use the validated tag without a raw-ref fallback"


def test_the_action_accepts_a_tag_and_a_rehearsal() -> None:
    """Declare both selectors, with the tag optional so a rehearsal may omit it.

    A still-required tag would make a pull-request run fail before reaching the
    mode logic, which is the failure this contract exists to prevent.
    """
    inputs = require_mapping(_action_manifest().get("inputs"), "action inputs")
    tag = require_mapping(inputs.get("tag"), "tag input")
    rehearsal = require_mapping(inputs.get("dry-run"), "dry-run input")

    assert tag.get("required") is not True, (
        "the tag must be optional, or a rehearsal cannot omit it"
    )
    assert rehearsal.get("default") == "false", (
        f"the rehearsal flag must default to false, got {rehearsal.get('default')!r}"
    )


#: Each manifest input that must reach the CLI: the input name, the environment
#: reference the step assigns it to, and the CLI flag that reads it back.
#: ``package-name`` and ``bin-name`` are the pair that differ — the crate is
#: ``netsuke-build`` while the binary it installs is ``netsuke`` — so
#: forwarding the wrong one produces a valid crate under a name the release
#: never declared.
ARGUMENT_FORWARDING = (
    (
        "commit",
        "RELEASE_COMMIT",
        "${{ inputs.commit }}",
        '--commit "$RELEASE_COMMIT"',
        "the commit must reach the CLI",
    ),
    (
        "package-name",
        "RELEASE_PACKAGE_NAME",
        "${{ inputs['package-name'] }}",
        '--package-name "$RELEASE_PACKAGE_NAME"',
        "the Cargo package name must reach the CLI",
    ),
    (
        "version",
        "RELEASE_VERSION",
        "${{ inputs.version }}",
        '--version "$RELEASE_VERSION"',
        "the version must reach the CLI",
    ),
    (
        "bin-name",
        "RELEASE_BIN_NAME",
        "${{ inputs['bin-name'] }}",
        '--bin-name "$RELEASE_BIN_NAME"',
        "the binary name must reach the CLI",
    ),
)

#: The repository-relative record the CLI writes, as the action spells it.
IDENTITY_PATH = "artifacts/package/identity.json"

#: The three outputs the action publishes, as the ``printf`` format strings
#: that write them. The body is a YAML literal block, so ``\n`` stays two
#: characters for the shell rather than becoming a newline here.
OUTPUT_FORMATS = (
    (r"printf 'identity-path=%s\n'", IDENTITY_PATH),
    (r"printf 'crate-name=%s\n'", "$crate_name"),
    (r"printf 'crate-path=%s\n'", "target/package/$crate_name"),
)


@pytest.mark.parametrize(
    ("mode", "expected"),
    [("true", "--dry-run"), ("false", '--tag "$RELEASE_TAG"')],
)
def test_the_action_selects_the_selector_before_invoking_the_cli(
    mode: str, expected: str
) -> None:
    """Parse each case arm and require the exact exclusive selector assignment."""
    run = _action_run_body()
    case = re.search(
        r'case "\$RELEASE_DRY_RUN" in\n(?P<arms>.*?)\n\s*esac', run, re.DOTALL
    )
    assert case is not None, "the action must select a mode using RELEASE_DRY_RUN"
    arms = case.group("arms")
    arm = re.search(rf"^\s*{mode}\)\s*(.*?)\s*;;\s*$", arms, re.MULTILINE)
    assert arm is not None, f"missing {mode!r} case arm"
    assert arm.group(1) == f"release_selector=({expected})", (
        f"mode {mode!r} must select only {expected!r}, got {arm.group(1)!r}"
    )
    assert '"${release_selector[@]}"' in run, (
        "the selected arguments must reach the CLI"
    )


@pytest.mark.parametrize(
    "case",
    ARGUMENT_FORWARDING,
    ids=[row[0] for row in ARGUMENT_FORWARDING],
)
def test_the_action_forwards_every_identity_field_to_the_cli(
    case: tuple[str, str, str, str, str],
) -> None:
    """Pass each declared input to the CLI under the flag the CLI parses.

    The inputs are read into named environment variables so the manifest can
    describe each one. That indirection is exactly where a rename goes
    unnoticed: the manifest still declares ``package-name``, the CLI still
    accepts ``--package-name``, and the value between them has become empty.
    Both ends are asserted here, plus the assignment joining them.
    """
    inputs, environment = _action_inputs_and_environment()

    assert case[0] in inputs, f"the action must declare a {case[0]!r} input"
    assert case[3] in _action_run_body(), case[4]
    assert environment.get(case[1]) == case[2], (
        f"the {case[0]!r} input must map exactly to {case[1]} as {case[2]!r}"
    )


def test_the_action_maps_the_record_onto_its_declared_outputs() -> None:
    """Publish the resolved archive name and both paths as step outputs.

    The job consuming this action resolves the archive from ``crate-path``, so
    an output that stopped being written would hand the next job an empty path
    and an upload step that stages nothing. Each ``printf`` is asserted with
    the value it interpolates, not merely the output name: the format string
    says which output, and the argument says what it is built from.
    """
    run = _action_run_body()

    assert f"jq -r '.crate' {IDENTITY_PATH}" in run, (
        "the archive name must be read from the record the CLI wrote"
    )
    for printf, argument in OUTPUT_FORMATS:
        assert printf in run, f"the action must write {printf!r} to its outputs"
        assert f"{printf} '{argument}'" in run or f'{printf} "{argument}"' in run, (
            f"the output written by {printf!r} must carry {argument!r}"
        )
    assert '>>"$GITHUB_OUTPUT"' in run, (
        "the captured values must be appended to the step output file"
    )


def test_the_action_refuses_an_unrecognised_mode() -> None:
    """Require the case fallback to diagnose and reject every other mode."""
    run = _action_run_body()
    case = re.search(
        r'case "\$RELEASE_DRY_RUN" in\n(?P<arms>.*?)\n\s*esac', run, re.DOTALL
    )
    assert case is not None, "the action must branch on RELEASE_DRY_RUN"
    fallback = re.search(
        r"^\s*\*\)\s*(.*?)\s*;;", case.group("arms"), re.DOTALL | re.MULTILINE
    )
    assert fallback is not None, "unknown modes must enter a catch-all arm"
    assert "dry-run must be true or false" in fallback.group(1), (
        "the catch-all must diagnose an unknown mode"
    )
    assert re.search(r"^\s*exit 1\s*$", fallback.group(1), re.MULTILINE), (
        "the catch-all must fail before invoking the CLI"
    )
