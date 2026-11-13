"""Read the declared-asset action's manifest and resolve what it names.

The execution cases drive the action's real shell bodies, so they start by
reading those bodies out of ``.github/actions/upload-declared-assets/action.yml``
rather than restating them. Reading them is what makes the cases hold the flag
spelling, the argument order, and the deferred ``upload-error`` contract to
account as written: a case that restated the command line would keep passing
after the manifest changed.

A step's ``env`` block is written in GitHub expressions. This module resolves
the handful of expressions the action uses into the values a runner would
substitute, and fails the case on any other expression, so an input added to
the manifest shows up here as an explicit refusal rather than as an unset
variable on a runner.

The staged tree those steps read is built by
``upload_declared_assets_staging.py``; the harness that runs the bodies is
``upload_declared_assets_harness.py``.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from action_run_sources import action_sources, action_steps, require_action_step
from upload_declared_assets_staging import COMMIT, DIST, Facts
from workflow_loading import require_mapping

#: The manifest the cases read their run bodies from.
ACTION = ".github/actions/upload-declared-assets/action.yml"

#: The steps that execute a script. The ``upload-artifact`` step between them
#: cannot run outside GitHub, so it is asserted structurally instead.
SELECT_STEP = "Select the declared release payload"
DECLARE_STEP = "Declare the release asset names"
RETAIN_STEP = "Retain the declared asset record"
UPLOAD_STEP = "Upload the declared release assets"
PLAN_STEP = "Retain the upload plan"
RUN_STEPS = (SELECT_STEP, DECLARE_STEP, UPLOAD_STEP)


def action_document() -> dict[str, object]:
    """Return the action's parsed manifest.

    Returns
    -------
    dict[str, object]
        The manifest as a mapping.
    """
    sources = action_sources()
    assert ACTION in sources, f"{ACTION} must be a composite action manifest"
    return sources[ACTION]


def manifest_step_id(step_name: str) -> str:
    """Return the ``id:`` the manifest gives one of its script steps.

    The id is read rather than assumed because GitHub keys a step's outputs by
    that id, and the action's ``outputs:`` block spells it in a
    ``steps.<id>.outputs.<name>`` expression. A case that hard-coded the id
    would keep passing after the manifest renamed the step, while the
    expressions in ``outputs:`` named an id no step carried and GitHub
    substituted an empty string.

    Parameters
    ----------
    step_name
        The ``name`` of the step whose id is wanted.

    Returns
    -------
    str
        The ``id`` the manifest declares for that step.
    """
    document = action_document()
    step = action_steps(ACTION, document)[
        require_action_step(ACTION, document, step_name)
    ]
    step_id = step.get("id")
    assert isinstance(step_id, str), (
        f"{ACTION} step {step_name!r} must declare an id: for its outputs"
    )
    assert step_id, f"{ACTION} step {step_name!r} must declare a non-empty id:"
    return step_id


def run_bodies() -> tuple[dict[str, object], ...]:
    """Return the manifest's script steps, in declaration order.

    Returns
    -------
    tuple[dict[str, object], ...]
        The selection, declaration, and upload steps.
    """
    document = action_document()
    steps = action_steps(ACTION, document)
    return tuple(
        steps[require_action_step(ACTION, document, name)] for name in RUN_STEPS
    )


def release_tag(facts: Facts) -> str:
    """Return the tag a release of ``facts.version`` carries.

    The metadata job derives the tag from the pushed ref, and the workflow
    admits only ``v*.*.*``, so the ``v`` prefix is all the fixture has to
    reproduce.

    Returns
    -------
    str
        ``v`` prefixed to the Cargo package version.
    """
    return f"v{facts.version}"


def _resolved(
    expression: str, *, facts: Facts, declared_root: Path, dry_run: str
) -> str:
    """Return the value GitHub would substitute for one step expression.

    Parameters
    ----------
    dry_run
        What the ``dry-run`` input is set to. Substituted rather than fixed
        because the caller chooses the mode, and the step's own ``case``
        statement is one of the things under test.

    Returns
    -------
    str
        The value the runner would substitute.
    """
    known = {
        "${{ inputs['dist-dir'] }}": DIST,
        "${{ inputs['staging-config'] }}": ".github/release-staging.toml",
        "${{ inputs.manifest }}": "Cargo.toml",
        "${{ inputs.version }}": facts.version,
        "${{ inputs.commit }}": COMMIT,
        "${{ inputs['release-tag'] }}": release_tag(facts),
        "${{ inputs['dry-run'] }}": dry_run,
    }
    if expression in known:
        return known[expression]
    if expression == "${{ runner.temp }}/declared-assets":
        return str(declared_root)
    # Any other expression is one the manifest has grown since this case was
    # written; failing loudly beats substituting a value the runner would not.
    unresolvable = f"{ACTION} names an expression this case cannot resolve"
    return pytest.fail(f"{unresolvable}: {expression}")


def step_environment(
    step: dict[str, object],
    *,
    facts: Facts,
    declared_root: Path,
    dry_run: str = "false",
) -> dict[str, str]:
    """Return one step's ``env`` block with every expression resolved.

    Returns
    -------
    dict[str, str]
        The step's environment variables.
    """
    declared = require_mapping(step.get("env"), f"{ACTION} step env")
    return {
        str(key): _resolved(
            str(value), facts=facts, declared_root=declared_root, dry_run=dry_run
        )
        for key, value in declared.items()
    }


if typ.TYPE_CHECKING:
    from pathlib import Path
