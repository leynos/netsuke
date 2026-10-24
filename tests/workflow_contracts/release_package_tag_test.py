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
)

if typ.TYPE_CHECKING:
    from pathlib import Path

PACKAGE_JOB = "package-crate"
PACKAGE_STEP = "Package the crate and record its identity"
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


#: Each fragment the action's ``run`` body must carry, with the reason it is
#: required. A table rather than three similar assertions, so a reader sees the
#: complete set of selectors the CLI's mutual exclusion depends on.
SELECTOR_FRAGMENTS = (
    ("--dry-run", "the action must be able to select a rehearsal"),
    ('--tag "$RELEASE_TAG"', "the action must select the release tag"),
    ("case", "the action must branch on the mode rather than pass both selectors"),
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
        "${{ inputs.commit }}",
        '--commit "$RELEASE_COMMIT"',
        "the commit must reach the CLI",
    ),
    (
        "package-name",
        "${{ inputs['package-name'] }}",
        '--package-name "$RELEASE_PACKAGE_NAME"',
        "the Cargo package name must reach the CLI",
    ),
    (
        "version",
        "${{ inputs.version }}",
        '--version "$RELEASE_VERSION"',
        "the version must reach the CLI",
    ),
    (
        "bin-name",
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
    ("fragment", "reason"),
    SELECTOR_FRAGMENTS,
    ids=[row[0] for row in SELECTOR_FRAGMENTS],
)
def test_the_action_selects_the_selector_before_invoking_the_cli(
    fragment: str, reason: str
) -> None:
    """Give the CLI exactly one of ``--tag`` and ``--dry-run``, never both.

    The CLI refuses the pair, so an action that passed both would fail every
    run; an action that passed neither would fail too. The body therefore has
    to branch, and the assertion is that both selector strings reach it.
    """
    run = _action_run_body()

    assert fragment in run, reason


@pytest.mark.parametrize(
    ("name", "reference", "flag", "reason"),
    ARGUMENT_FORWARDING,
    ids=[row[0] for row in ARGUMENT_FORWARDING],
)
def test_the_action_forwards_every_identity_field_to_the_cli(
    name: str, reference: str, flag: str, reason: str
) -> None:
    """Pass each declared input to the CLI under the flag the CLI parses.

    The inputs are read into named environment variables so the manifest can
    describe each one. That indirection is exactly where a rename goes
    unnoticed: the manifest still declares ``package-name``, the CLI still
    accepts ``--package-name``, and the value between them has become empty.
    Both ends are asserted here, plus the assignment joining them.
    """
    run = _action_run_body()
    manifest = _action_manifest()
    inputs = require_mapping(manifest.get("inputs"), "action inputs")

    assert name in inputs, f"the action must declare a {name!r} input"
    assert flag in run, reason
    assert reference in str(manifest), (
        f"the {name!r} input must be assigned to the step environment as {reference!r}"
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
    """Fail the step on any mode other than ``true`` or ``false``.

    The ``case`` has a branch for each real mode. Without a catch-all, an empty
    or malformed value would fall through with no selector set and the CLI
    would report a missing argument instead of the input that caused it.
    """
    run = _action_run_body()

    assert "dry-run must be true or false" in run, (
        "an unrecognised mode must be named rather than fall through"
    )
    assert "exit 1" in run, "an unrecognised mode must fail the step"
