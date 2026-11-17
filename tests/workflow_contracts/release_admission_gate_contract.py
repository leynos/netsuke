"""Require the admission gate's enforcement mode to follow the release mode.

A gate that observed on a publishing run reports success while admitting
nothing, and a gate that enforced on a rehearsal makes the read-only lane
unreachable. The workflow therefore derives enforcement from `should_publish`
rather than configuring it beside the mode. This module evaluates that
expression in every scenario instead of trusting its text, so a hard-coded mode
in either direction is reported for the scenario that exposes it.

The job must also run the gate from this repository's own commit: a reusable
workflow's default checkout resolves the caller's repository, so an unpinned
checkout would let a caller supply a gate that reports whatever the write jobs
want to hear. :func:`check_admission_source_pin` holds that pin.

The write jobs that consume the mode are held to it in
:mod:`release_write_authority_contract`.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_publish_path_artifacts import MISSING, FieldEvaluation, evaluate_field
from release_publish_path_scenarios import GITHUB_SHA
from workflow_loading import job_steps, require_mapping

if typ.TYPE_CHECKING:
    from release_publish_path_scenarios import Scenario

ADMISSION_JOB = "release-admission-canaries"
ADMISSION_STEP_ID = "release_admission"
ENFORCE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_ENFORCE"
#: The repository whose commit defines the gate implementation. The admission
#: job must check this out rather than the caller's repository: a reusable
#: workflow's default checkout resolves the caller, so an unpinned job runs
#: whatever gate the caller's candidate contains.
GATE_REPOSITORY = "leynos/netsuke"
#: The ref that names the workflow's own commit. `job.workflow_sha` is the
#: commit of the workflow file that defines this job, which is a commit of this
#: repository both for a reusable-workflow call and for a directly triggered
#: run. A fallback to `github.sha` would not do: on a call it is the caller's
#: candidate, which is the commit the pin exists to refuse.
GATE_REF = "${{ job.workflow_sha }}"
CHECKOUT_ACTION = "actions/checkout"


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


def check_admission_source_pin(
    release: dict[str, object], violations: list[str]
) -> None:
    """Require the gate to run from this repository's own commit.

    The gate's mode, outcome and candidate outputs are what every write job
    trusts, so the code that produces them must not come from the caller. In a
    reusable workflow the default checkout resolves the caller's repository, so
    the job pins both the repository and the ref instead, and the gate step
    must run after that checkout.

    Parameters
    ----------
    release
        Parsed release workflow.
    violations
        Collector for the job's findings.
    """
    steps = job_steps(release, ADMISSION_JOB)
    label = f"{ADMISSION_JOB}.checkout"
    checkout = _sole_checkout(steps, label, violations)
    if checkout is None:
        return
    _check_gate_follows(steps, checkout, label, violations)
    declared = steps[checkout].get("with")
    if not isinstance(declared, dict):
        violations.append(f"{label}: the checkout must declare repository and ref")
        return
    _check_repository(declared, label, violations)
    _check_ref(declared, label, violations)


def _sole_checkout(
    steps: list[dict[str, object]], label: str, violations: list[str]
) -> int | None:
    """Return the one checkout step's index, reporting any other count."""
    checkouts = [
        index
        for index, step in enumerate(steps)
        if str(step.get("uses", "")).startswith(f"{CHECKOUT_ACTION}@")
    ]
    if len(checkouts) != 1:
        violations.append(
            f"{label}: expected exactly one {CHECKOUT_ACTION} step, "
            f"found {len(checkouts)}"
        )
        return None
    return checkouts[0]


def _check_gate_follows(
    steps: list[dict[str, object]], checkout: int, label: str, violations: list[str]
) -> None:
    """Require the gate step to run after the checkout that supplies it."""
    gate_index = next(
        (
            index
            for index, step in enumerate(steps)
            if step.get("id") == ADMISSION_STEP_ID
        ),
        None,
    )
    if gate_index is None or gate_index < checkout:
        violations.append(
            f"{label}: the checkout must precede the {ADMISSION_STEP_ID!r} step"
        )


def _check_repository(
    declared: dict[str, object], label: str, violations: list[str]
) -> None:
    """Require the checkout to name this repository, not the caller's."""
    if declared.get("repository") != GATE_REPOSITORY:
        violations.append(
            f"{label}: repository must be {GATE_REPOSITORY!r} so the gate is not "
            f"the caller's, got {declared.get('repository')!r}"
        )


def _check_ref(declared: dict[str, object], label: str, violations: list[str]) -> None:
    """Require the checkout to resolve this workflow's own commit."""
    if " ".join(str(declared.get("ref", "")).split()) != GATE_REF:
        violations.append(
            f"{label}: ref must be {GATE_REF!r} so the gate comes from the "
            f"workflow's own commit, got {declared.get('ref')!r}"
        )


def _resolved_flag(value: object) -> bool | None:
    """Interpret a step environment value the way the runner would.

    A step environment is a string on the runner, so a hard-coded `true` or
    `false` reaches the gate as text.

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
        case str() as text if text.strip().lower() in {"true", "false"}:
            return text.strip().lower() == "true"
        case _:
            return None


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
                "result": scenario.needs_results.get("metadata", "skipped"),
                "outputs": dict(metadata_outputs),
            }
        },
        "github": {"event": {"action": scenario.event_action}, "sha": GITHUB_SHA},
    }
    value = evaluate_field(
        expression,
        FieldEvaluation(contexts=contexts, violations=violations, label=label),
    )
    if value is MISSING:
        return
    resolved = _resolved_flag(value)
    expected = scenario.mode == "publish"
    if resolved is None:
        violations.append(f"{label}: resolved to {value!r}, which is not a flag")
    elif resolved != expected:
        violations.append(f"{label}: resolved to {resolved!r}, expected {expected!r}")


__all__ = (
    "ADMISSION_JOB",
    "ADMISSION_STEP_ID",
    "ENFORCE_VARIABLE",
    "GATE_REF",
    "GATE_REPOSITORY",
    "check_admission_enforcement",
    "check_admission_source_pin",
)
