"""Read the consumer-smoke action's manifest and resolve what it hands a step.

The execution cases drive the action's real shell bodies, so they start by
reading those bodies out of ``.github/actions/consumer-smoke/action.yml``
rather than restating them. Reading them is what holds the flag spelling, the
argument order, and the report step's phase labels to account as written: a
case that restated a command line would keep passing after the manifest
changed.

A step's ``env`` block is written in GitHub expressions. This module resolves
the three forms the action uses -- an input, a step output, and a step outcome
-- into the values a runner would substitute, and fails the case on any other
expression, so an input added to the manifest shows up here as an explicit
refusal rather than as an unset variable on a runner.

The harness that runs the bodies and the fixtures it publishes live in
``consumer_smoke_action_harness.py``.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ

import pytest
from action_run_sources import action_sources, action_steps, require_action_step
from release_publish_path_consumer_shape import CONSUMER_SMOKE_ACTION_PATH as ACTION

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The manifest step that reads the action's inputs and creates its workspace.
VALIDATE_STEP = "Validate inputs"
#: The manifest step that runs the checked-in planner.
PLAN_STEP = "Plan the public downloads"
#: The manifest step whose ``curl`` stand-in is the only network access.
DOWNLOAD_STEP = "Download the public assets"
#: The manifest step that runs the checked-in verifier.
VERIFY_STEP = "Verify the downloads and the shipped executable"
#: The manifest step that publishes the phases and the verdict path.
REPORT_STEP = "Report the smoke phases"
#: Every step, in the order a runner starts them.
RUN_STEPS = (VALIDATE_STEP, PLAN_STEP, DOWNLOAD_STEP, VERIFY_STEP, REPORT_STEP)

#: The condition a step uses to run even after an earlier refusal.
ALWAYS = "always()"

#: One GitHub expression, captured so the body can be resolved or refused.
_EXPRESSION = re.compile(r"\$\{\{\s*(?P<body>[^}]*?)\s*\}\}")
#: The two spellings an input reference may take; both are in the manifest.
_INPUT = re.compile(r"^inputs\.(?P<plain>[\w-]+)$|^inputs\['(?P<quoted>[\w-]+)'\]$")
#: A step's output reference, which names the id and the output together.
_STEP_OUTPUT = re.compile(r"^steps\.(?P<id>[\w-]+)\.outputs\.(?P<name>[\w-]+)$")
#: A step's outcome, which is what the report step is handed.
_STEP_OUTCOME = re.compile(r"^steps\.(?P<id>[\w-]+)\.outcome$")
#: The one step condition the action carries, other than ``always()``.
_OUTCOME_GUARD = re.compile(r"^steps\.(?P<id>[\w-]+)\.outcome\s*==\s*'success'$")


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


def smoke_steps() -> tuple[dict[str, object], ...]:
    """Return the manifest's steps, in declaration order.

    Returns
    -------
    tuple[dict[str, object], ...]
        Every step the action declares.
    """
    document = action_document()
    return tuple(action_steps(ACTION, document))


def _step_named(name: str) -> dict[str, object]:
    """Return one named step of the manifest as a mapping."""
    document = action_document()
    return action_steps(ACTION, document)[require_action_step(ACTION, document, name)]


def manifest_step_id(step_name: str) -> str:
    """Return the ``id:`` the manifest gives one of its steps.

    The id is read rather than assumed because GitHub keys a step's outputs by
    that id, and the action's ``outputs:`` block spells it in a
    ``steps.<id>.outputs.<name>`` expression.

    Returns
    -------
    str
        The id GitHub keys the step's outputs by.
    """
    step_id = _step_named(step_name).get("id")
    assert isinstance(step_id, str), (
        f"{ACTION} step {step_name!r} must declare an id: for its outputs"
    )
    assert step_id, f"{ACTION} step {step_name!r} must declare a non-empty id:"
    return step_id


def _resolve_body(
    body: str,
    *,
    inputs: dict[str, str],
    outputs: dict[str, dict[str, str]],
    outcomes: dict[str, str],
) -> str:
    """Resolve one expression body into the value a runner would substitute.

    Returns
    -------
    str
        The resolved value.

    Notes
    -----
    An expression this harness does not model is refused rather than left
    unresolved: a case that silently substituted an empty string would assert
    against a step that never read the input it names.
    """
    if (match := _INPUT.match(body)) is not None:
        name = match.group("plain") or match.group("quoted")
        assert name in inputs, f"the action reads an input no case supplies: {name!r}"
        return inputs[name]
    if (match := _STEP_OUTPUT.match(body)) is not None:
        return _step_output(match.group("id"), match.group("name"), outputs)
    if (match := _STEP_OUTCOME.match(body)) is not None:
        return outcomes[match.group("id")]
    return pytest.fail(f"the harness does not model the expression {body!r}")


def _step_output(step_id: str, name: str, outputs: dict[str, dict[str, str]]) -> str:
    """Return one step's output, refusing an id or name that never existed.

    Returns
    -------
    str
        The value the step wrote.
    """
    assert step_id in outputs, f"no step {step_id!r} has run yet"
    assert name in outputs[step_id], f"step {step_id!r} never wrote {name!r}"
    return outputs[step_id][name]


def resolve(
    value: object,
    *,
    inputs: dict[str, str],
    outputs: dict[str, dict[str, str]],
    outcomes: dict[str, str],
) -> str:
    """Resolve every GitHub expression in one environment value.

    Returns
    -------
    str
        The value with each expression replaced by what it names.
    """
    return _EXPRESSION.sub(
        lambda match: _resolve_body(
            match.group("body"), inputs=inputs, outputs=outputs, outcomes=outcomes
        ),
        str(value),
    )


def step_environment(
    step: dict[str, object],
    *,
    inputs: dict[str, str],
    outputs: dict[str, dict[str, str]],
    outcomes: dict[str, str],
) -> dict[str, str]:
    """Return the environment one step declares, expressions resolved.

    Returns
    -------
    dict[str, str]
        The step's own variables, ready to merge into the inherited ones.
    """
    declared = step.get("env")
    if not isinstance(declared, dict):
        return {}
    return {
        str(name): resolve(value, inputs=inputs, outputs=outputs, outcomes=outcomes)
        for name, value in declared.items()
    }


def should_run(
    step: dict[str, object], *, outcomes: dict[str, str], previous: cabc.Sequence[str]
) -> bool:
    """Report whether a runner would start ``step``.

    Returns
    -------
    bool
        True when the step's condition holds. A step without one inherits the
        runner's implicit ``success()``, so it starts only when every step
        before it succeeded.
    """
    condition = step.get("if")
    if condition is None:
        return all(outcome == "success" for outcome in previous)
    text = str(condition).strip()
    if text == ALWAYS:
        return True
    match = _OUTCOME_GUARD.match(text)
    assert match is not None, f"the harness does not model the condition {text!r}"
    return outcomes.get(match.group("id")) == "success"


__all__ = [
    "ALWAYS",
    "DOWNLOAD_STEP",
    "PLAN_STEP",
    "REPORT_STEP",
    "RUN_STEPS",
    "VALIDATE_STEP",
    "VERIFY_STEP",
    "action_document",
    "manifest_step_id",
    "resolve",
    "should_run",
    "smoke_steps",
    "step_environment",
]
