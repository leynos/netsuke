"""Break the admission job's two checkouts one clause at a time.

The admission job checks out two commits that must not be confused: the
caller's candidate at the workspace root, which is what leaves `origin` naming
the repository the candidate belongs to, and the pinned verifier in a
subdirectory, which is what keeps the gate implementation out of the caller's
hands. Each mutation here states a plausible way to lose one of those
properties -- collapse the pair into one checkout, let the verifier replace the
candidate, point either checkout at the other's repository or ref, run the
wrong copy of the entry point, or move the gate step out of the workspace root
-- so the mutation test shows each pin assertion fires rather than merely
passing.

The mutations are exported as :data:`CHECKOUT_MUTATIONS` and merged into the
publication gate's mutation set in :mod:`release_publication_gate_mutations`.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_admission_gate_contract import (
    ADMISSION_JOB,
    ADMISSION_STEP_ID,
    CHECKOUT_ACTION,
    GATE_REF,
    GATE_REPOSITORY,
    GATE_SCRIPT,
)
from release_mutation_support import MutationError, MutationIssue
from workflow_loading import job_steps, require_mapping, workflow_job

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def _checkout_with_path(
    release: dict[str, object], *, has_path: bool
) -> dict[str, object]:
    """Return the checkout step with, or without, a ``path`` input.

    The admission job declares two checkouts and the ``path`` input is what
    tells its roles apart: the candidate checkout sits at the workspace root so
    `origin` names the caller's repository, and the verifier checkout sits in a
    subdirectory so it cannot replace it.

    Returns
    -------
    dict[str, object]
        The first checkout step whose inputs do, or do not, declare a ``path``.

    Raises
    ------
    MutationError
        If no checkout has the requested shape, so the case the mutation
        describes no longer exists in this workflow.
    """
    for step in job_steps(release, ADMISSION_JOB):
        if not str(step.get("uses", "")).startswith(f"{CHECKOUT_ACTION}@"):
            continue
        declared = step.get("with")
        if isinstance(declared, dict) and ("path" in declared) is has_path:
            return step
    shape = "with a path" if has_path else "at the workspace root"
    raise MutationError(MutationIssue.PATTERN_MISMATCH, f"checkout {shape}")


def _checkout_inputs(
    release: dict[str, object], *, has_path: bool
) -> dict[str, object]:
    """Return one checkout's inputs, refusing a malformed declaration."""
    step = _checkout_with_path(release, has_path=has_path)
    return require_mapping(step.get("with"), f"{ADMISSION_JOB} checkout inputs")


def _gate_step(release: dict[str, object]) -> dict[str, object]:
    """Return the gate step, refusing a workflow that no longer declares it."""
    step = next(
        (
            item
            for item in job_steps(release, ADMISSION_JOB)
            if item.get("id") == ADMISSION_STEP_ID
        ),
        None,
    )
    if step is None:
        raise MutationError(MutationIssue.PATTERN_MISMATCH, ADMISSION_STEP_ID)
    return step


def _caller_selects_gate(release: dict[str, object]) -> None:
    """Let the verifier checkout resolve whatever the caller's candidate has.

    This is the shape an unpinned ``actions/checkout`` produces in a reusable
    workflow: the job checks out the caller's repository, so a caller that
    keeps an older gate -- one written before the producer-trust refusal -- can
    emit the mode, outcome and candidate the write guards consume while
    ignoring the shipped literal.

    Raises
    ------
    MutationError
        If the verifier checkout no longer declares the inputs this mutation
        removes, so the shape it describes is already absent.
    """
    declared = _checkout_inputs(release, has_path=True)
    for key in ("repository", "ref"):
        if declared.pop(key, None) is None:
            raise MutationError(MutationIssue.PATTERN_MISMATCH, key)


def _gate_from_caller_repository(release: dict[str, object]) -> None:
    """Name the caller's repository while keeping the trusted ref."""
    _checkout_inputs(release, has_path=True)["repository"] = "${{ github.repository }}"


def _gate_from_run_commit_only(release: dict[str, object]) -> None:
    """Name the run's commit instead of the commit that defines the job.

    On a reusable-workflow call ``github.sha`` is the caller's candidate, so an
    older gate committed there would run while the repository input still names
    this one.
    """
    _checkout_inputs(release, has_path=True)["ref"] = "${{ github.sha }}"


def _single_pinned_checkout(release: dict[str, object]) -> None:
    """Keep the pinned verifier and drop the candidate checkout.

    This is the shape that re-points `origin` at this repository: the gate then
    fetches the candidate revision from here, so a caller whose commit this
    repository does not have is refused before its evidence is even read.
    """
    steps = job_steps(release, ADMISSION_JOB)
    steps.remove(_checkout_with_path(release, has_path=False))


def _gate_checkout_replaces_candidate(release: dict[str, object]) -> None:
    """Let the verifier checkout write to the workspace root."""
    declared = _checkout_inputs(release, has_path=True)
    if declared.pop("path", None) is None:
        raise MutationError(MutationIssue.PATTERN_MISMATCH, "path")


def _candidate_checkout_names_verifier(release: dict[str, object]) -> None:
    """Make the candidate checkout select this repository's defining commit."""
    declared = _checkout_inputs(release, has_path=False)
    declared["repository"] = GATE_REPOSITORY
    declared["ref"] = GATE_REF


def _gate_runs_candidate_script(release: dict[str, object]) -> None:
    """Run the entry point from the candidate checkout instead of the verifier."""
    _gate_step(release)["run"] = f"uv run --script {GATE_SCRIPT}"


def _gate_step_leaves_candidate(release: dict[str, object]) -> None:
    """Run the gate from the verifier checkout, which re-points `origin`."""
    declared = _checkout_inputs(release, has_path=True)
    _gate_step(release)["working-directory"] = declared["path"]


def _job_default_moves_gate(release: dict[str, object]) -> None:
    """Move every run step of the job into the verifier checkout."""
    declared = _checkout_inputs(release, has_path=True)
    workflow_job(release, ADMISSION_JOB)["defaults"] = {
        "run": {"working-directory": declared["path"]}
    }


#: Each mutation that breaks the candidate/verifier split, keyed by its label.
CHECKOUT_MUTATIONS: dict[str, cabc.Callable[[dict[str, object]], None]] = {
    "caller-selects-gate-implementation": _caller_selects_gate,
    "gate-from-caller-repository": _gate_from_caller_repository,
    "gate-from-run-commit-only": _gate_from_run_commit_only,
    "single-pinned-checkout": _single_pinned_checkout,
    "gate-checkout-replaces-candidate": _gate_checkout_replaces_candidate,
    "candidate-checkout-names-verifier": _candidate_checkout_names_verifier,
    "gate-runs-candidate-script": _gate_runs_candidate_script,
    "gate-step-leaves-candidate": _gate_step_leaves_candidate,
    "job-default-moves-gate": _job_default_moves_gate,
}

__all__ = ("CHECKOUT_MUTATIONS",)
