"""Read the inventory verifier's manifest and resolve what its steps name.

The execution cases drive the action's real shell bodies, so they start by
reading those bodies out of ``.github/actions/verify-release-inventory/action.yml``
rather than restating them. Reading them is what holds the flag spelling, the
argument order, and the ``verified-record-path`` output to account as written: a
case that restated the command line would keep passing after the manifest
changed.

A step's ``env`` block is written in GitHub expressions. This module resolves
the handful of expressions the action uses into the values a runner would
substitute, and fails the case on any other expression, so an input added to the
manifest shows up here as an explicit refusal rather than as an unset variable
on a runner.

The harness that runs the bodies is ``verify_release_inventory_harness.py``;
the stand-ins it installs are in ``verify_release_inventory_stubs.py``.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from action_run_sources import action_sources, action_steps, require_action_step
from workflow_loading import require_mapping

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

#: The manifest the cases read their run bodies from.
ACTION = ".github/actions/verify-release-inventory/action.yml"

VALIDATE_STEP = "Validate inputs"
LIST_RELEASES_STEP = "List draft releases"
LIST_ASSETS_STEP = "List draft assets"
DOWNLOAD_STEP = "Download draft assets"
VERIFY_STEP = "Verify inventory"
RUN_STEPS = (
    VALIDATE_STEP,
    LIST_RELEASES_STEP,
    LIST_ASSETS_STEP,
    DOWNLOAD_STEP,
    VERIFY_STEP,
)


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


def run_bodies() -> tuple[dict[str, object], ...]:
    """Return the manifest's script steps, in declaration order.

    Returns
    -------
    tuple[dict[str, object], ...]
        Every step that executes a script.
    """
    document = action_document()
    steps = action_steps(ACTION, document)
    return tuple(
        steps[require_action_step(ACTION, document, name)] for name in RUN_STEPS
    )


def _input_expressions(values: cabc.Mapping[str, str]) -> dict[str, str]:
    """Return both spellings GitHub accepts for every supplied input.

    A dotted name and a bracketed string name are the same input, and the
    manifest uses each where it reads better, so both are resolved rather than
    the case having to remember which step chose which.

    Returns
    -------
    dict[str, str]
        Each expression mapped to the value the runner would substitute.
    """
    known: dict[str, str] = {}
    for name, value in values.items():
        known[f"${{{{ inputs.{name} }}}}"] = value
        known[f"${{{{ inputs['{name}'] }}}}"] = value
    return known


def _resolved(
    expression: str, values: cabc.Mapping[str, str], inventory_root: Path
) -> str:
    """Return the value GitHub would substitute for one step expression.

    Returns
    -------
    str
        The value the runner would substitute.
    """
    known = _input_expressions(values)
    if expression in known:
        return known[expression]
    if expression == "${{ runner.temp }}/release-inventory":
        return str(inventory_root)
    # Any other expression is one the manifest has grown since this case was
    # written; failing loudly beats substituting a value the runner would not.
    unresolvable = f"{ACTION} names an expression this case cannot resolve"
    return pytest.fail(f"{unresolvable}: {expression}")


def step_environment(
    step: dict[str, object],
    *,
    values: cabc.Mapping[str, str],
    inventory_root: Path,
) -> dict[str, str]:
    """Return one step's ``env`` block with every expression resolved.

    Returns
    -------
    dict[str, str]
        The step's environment variables.
    """
    declared = require_mapping(step.get("env"), f"{ACTION} step env")
    return {
        str(key): _resolved(str(value), values, inventory_root)
        for key, value in declared.items()
    }
