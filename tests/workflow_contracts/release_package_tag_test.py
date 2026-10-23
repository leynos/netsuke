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
