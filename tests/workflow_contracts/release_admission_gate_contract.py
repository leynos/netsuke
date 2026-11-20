"""Require the admission gate's enforcement mode to follow the release mode.

A gate that observed on a publishing run reports success while admitting
nothing, and a gate that enforced on a rehearsal makes the read-only lane
unreachable. The workflow therefore derives enforcement from `should_publish`
rather than configuring it beside the mode. This module evaluates that
expression in every scenario instead of trusting its text, so a hard-coded mode
in either direction is reported for the scenario that exposes it.

The job must also run the gate from this repository's own commit while fetching
the candidate from the caller's. :mod:`release_admission_source_pin` holds that
two-checkout shape; this module re-exports its names so callers that treat the
admission contract as one surface keep a single import, and the write jobs that
consume the mode are held to it in :mod:`release_write_authority_contract`.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_admission_source_pin import (
    ADMISSION_JOB,
    ADMISSION_STEP_ID,
    CANDIDATE_REF,
    CANDIDATE_REPOSITORY,
    CHECKOUT_ACTION,
    GATE_REF,
    GATE_REPOSITORY,
    GATE_SCRIPT,
    PINNED_UV_VERSION,
    SETUP_UV_ACTION,
    UV_ISOLATION_FLAGS,
    check_admission_source_pin,
    verifier_gate_command,
)
from release_publish_path_artifacts import MISSING, FieldEvaluation, evaluate_field
from release_publish_path_scenario_model import GITHUB_SHA
from workflow_loading import job_steps, require_mapping

if typ.TYPE_CHECKING:
    from release_publish_path_scenario_model import Scenario

ENFORCE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_ENFORCE"


def _admission_step(
    release: dict[str, object], violations: list[str]
) -> dict[str, object]:
    """Return the gate step, or report a job that no longer declares it."""
    step = next(
        (
            item
            for item in job_steps(release, ADMISSION_JOB)
            if item.get("id") == ADMISSION_STEP_ID
        ),
        None,
    )
    if step is None:
        violations.append(
            f"{ADMISSION_JOB}.steps: the {ADMISSION_STEP_ID!r} step is missing"
        )
        return {}
    return step


def _resolved_flag(value: object) -> bool | None:
    """Interpret a step environment value the way the runner and gate would.

    A step environment is a string on the runner, so a boolean expression's
    result arrives as lowercase ``true`` or ``false``. The gate compares that
    text against those exact spellings, so ``TRUE`` or a padded value is not a
    flag: a publishing run would receive it and observe, admitting nothing.

    Returns
    -------
    bool | None
        The flag the value denotes, or ``None`` when it is not a flag at all.
        The distinction matters: an unreadable value must fail, not count as
        "not enforcing" and thereby satisfy a rehearsal's expectation by
        accident.
    """
    match value:
        case bool() as flag:
            return flag
        case "true":
            return True
        case "false":
            return False
        case _:
            return None


def _is_expression(text: str) -> bool:
    """Return whether a step environment value is a wrapped expression.

    Returns
    -------
    bool
        Whether the value is entirely one ``${{ }}`` expression. Anything else
        is literal text on the runner.
    """
    stripped = text.strip()
    return stripped.startswith("${{") and stripped.endswith("}}")


def _environment_value(
    expression: object,
    contexts: dict[str, object],
    label: str,
    violations: list[str],
) -> object:
    """Return the value the runner assigns to the gate's environment.

    A step environment is interpolated only where it is wrapped in ``${{ }}``;
    unwrapped text reaches the gate literally. Evaluating bare text as an
    expression would accept wiring whose delimiters were lost, which leaves the
    gate comparing an expression's own source against the mode.

    Returns
    -------
    object
        The evaluated expression, the literal text, or ``MISSING`` when the
        evaluator refuses the expression it was given.
    """
    if isinstance(expression, str) and not _is_expression(expression):
        return expression
    return evaluate_field(
        expression,
        FieldEvaluation(contexts=contexts, violations=violations, label=label),
    )


def check_admission_enforcement(
    release: dict[str, object],
    scenario: Scenario,
    metadata_outputs: dict[str, str],
    violations: list[str],
) -> None:
    """Require enforcement exactly where the scenario publishes.

    Parameters
    ----------
    release
        Parsed release workflow.
    scenario
        The release mode and dependency outcomes under test.
    metadata_outputs
        The metadata outputs the gate's expression reads.
    violations
        Collector for every scenario's findings.
    """
    step = _admission_step(release, violations)
    if not step:
        return
    environment = require_mapping(step.get("env"), f"{ADMISSION_JOB} step environment")
    expression = environment.get(ENFORCE_VARIABLE, MISSING)
    label = f"{scenario.name}.admission.enforcement"
    if expression is MISSING:
        violations.append(f"{label}: {ENFORCE_VARIABLE} is missing")
        return
    contexts: dict[str, object] = {
        "needs": {
            "metadata": {
                "result": scenario.effective_results.get("metadata", "skipped"),
                "outputs": dict(metadata_outputs),
            }
        },
        "github": {"event": {"action": scenario.event_action}, "sha": GITHUB_SHA},
    }
    value = _environment_value(expression, contexts, label, violations)
    if value is MISSING:
        return
    resolved = _resolved_flag(value)
    # Enforcement follows the policy's publication decision, not the
    # resolver's nominal mode: a reusable push the policy narrows to a
    # rehearsal must observe even though the resolver said publish.
    expected = metadata_outputs.get("should_publish") == "true"
    if resolved is None:
        violations.append(f"{label}: resolved to {value!r}, which is not a flag")
    elif resolved != expected:
        violations.append(f"{label}: resolved to {resolved!r}, expected {expected!r}")


__all__ = (
    "ADMISSION_JOB",
    "ADMISSION_STEP_ID",
    "CANDIDATE_REF",
    "CANDIDATE_REPOSITORY",
    "CHECKOUT_ACTION",
    "ENFORCE_VARIABLE",
    "GATE_REF",
    "GATE_REPOSITORY",
    "GATE_SCRIPT",
    "PINNED_UV_VERSION",
    "SETUP_UV_ACTION",
    "UV_ISOLATION_FLAGS",
    "check_admission_enforcement",
    "check_admission_source_pin",
    "verifier_gate_command",
)
