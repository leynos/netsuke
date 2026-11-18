"""Break the admission job's two checkouts one clause at a time.

The admission job checks out two commits that must not be confused: the
caller's candidate at the workspace root, which is what leaves `origin` naming
the repository the candidate belongs to, and the pinned verifier in a
subdirectory, which is what keeps the gate implementation out of the caller's
hands. Each mutation here states a plausible way to lose one of those
properties -- collapse the pair into one checkout, let the verifier replace the
candidate, point either checkout at the other's repository or ref, make a
checkout conditional or failure-tolerant, let the verifier keep its credential,
give the verifier a path the shell would split, write the verifier first, run
the wrong copy of the entry point, split the gate command at a shell command
boundary, drop uv's isolation from the candidate's configuration, or move the
gate step out of the workspace root -- at the step, the job, or the workflow
level -- so the mutation test shows each pin assertion fires rather than merely
passing.

The mutations are exported as :data:`CHECKOUT_MUTATIONS` and merged into the
publication gate's mutation set in :mod:`release_publication_gate_mutations`.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

from release_admission_gate_contract import (
    ADMISSION_JOB,
    ADMISSION_STEP_ID,
    CHECKOUT_ACTION,
    GATE_REF,
    GATE_REPOSITORY,
    GATE_SCRIPT,
    UV_ISOLATION_FLAGS,
)
from release_mutation_support import MutationError, MutationIssue
from workflow_loading import job_steps, require_mapping, workflow_job

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The two isolation flags as one adjacent span, separated by whatever
#: whitespace the command uses. Matching the pair rather than the literal
#: string means a reflowed command is still mutated, and requiring the pair
#: keeps the mutation from removing one flag and leaving the other.
_CONFIG_FLAG, _PROJECT_FLAG = UV_ISOLATION_FLAGS.split()
_UV_ISOLATION_SPAN = re.compile(
    rf"(?<=\s){re.escape(_CONFIG_FLAG)}[ \t]+{re.escape(_PROJECT_FLAG)}(?=\s|$)"
)


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


def _make_conditional(release: dict[str, object], *, has_path: bool) -> None:
    """Give one checkout an ``if`` that can skip it.

    The key is the defect, not the value: an ``if`` accepts an expression, so
    no literal proves the step always runs. A skipped checkout leaves the gate
    step looking for its script in whatever remains of the workspace.
    """
    _checkout_with_path(release, has_path=has_path)["if"] = "false"


def _tolerate_checkout_failure(release: dict[str, object], *, has_path: bool) -> None:
    """Let one checkout fail without stopping the job.

    The failure is tolerated rather than reported, so the gate step runs on
    whatever the checkout left behind -- which for a failed checkout is the
    other role's content, or none at all.
    """
    _checkout_with_path(release, has_path=has_path)["continue-on-error"] = True


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


def _candidate_checkout_conditional(release: dict[str, object]) -> None:
    """Let the candidate checkout be skipped.

    Without it the workspace root is not a checkout of the caller at all, so
    the gate's fetch of the candidate revision has no `origin` that names the
    caller's repository, and a caller whose commit this repository does not
    have can never be fetched.
    """
    _make_conditional(release, has_path=False)


def _candidate_checkout_tolerates_failure(release: dict[str, object]) -> None:
    """Let a failed candidate checkout pass unnoticed.

    The job continues with a partial workspace whose `origin` may not be the
    caller's, so the fetch the gate reports on is not the fetch the candidate
    requires.
    """
    _tolerate_checkout_failure(release, has_path=False)


def _verifier_checkout_conditional(release: dict[str, object]) -> None:
    """Let the verifier checkout be skipped.

    The gate command still names ``<path>/.github/scripts/release_admission.py``
    relative to the workspace root, and the candidate checkout owns that root:
    a candidate commit can carry the same path and supply the verifier the pin
    exists to refuse.
    """
    _make_conditional(release, has_path=True)


def _verifier_checkout_tolerates_failure(release: dict[str, object]) -> None:
    """Let a failed verifier checkout pass unnoticed.

    A checkout that failed may have written nothing at the verifier's path, and
    the candidate's own content can occupy it, which is the same substitution a
    skipped verifier allows.
    """
    _tolerate_checkout_failure(release, has_path=True)


def _verifier_persists_credentials(release: dict[str, object]) -> None:
    """Leave the verifier checkout's credential in the candidate's workspace.

    The verifier's token would then sit inside the tree the candidate's content
    occupies, where any process the gate runs from the workspace root can read
    it.

    Raises
    ------
    MutationError
        If the checkout already persists credentials, so the mutation would
        change nothing the contract reads.
    """
    declared = _checkout_inputs(release, has_path=True)
    if declared.get("persist-credentials") is not False:
        raise MutationError(
            MutationIssue.PATTERN_MISMATCH, "persist-credentials: false"
        )
    declared["persist-credentials"] = True


def _verifier_path_needs_quoting(release: dict[str, object]) -> None:
    """Move the verifier to a directory the gate command cannot pass as one word.

    The gate command interpolates the path unquoted, so a space splits it into
    two arguments and the gate never starts -- a job that reports success
    without judging anything.

    Raises
    ------
    MutationError
        If the verifier checkout declares no path, so there is nothing to
        split.
    """
    declared = _checkout_inputs(release, has_path=True)
    path = declared.get("path")
    if not isinstance(path, str) or not path:
        raise MutationError(MutationIssue.PATTERN_MISMATCH, "path")
    declared["path"] = f"{path} files"


def _gate_runs_candidate_script(release: dict[str, object]) -> None:
    """Run the entry point from the candidate checkout instead of the verifier."""
    _gate_step(release)["run"] = f"uv run --script {GATE_SCRIPT}"


def _checkouts_swapped(release: dict[str, object]) -> None:
    """Write the verifier checkout before the candidate checkout.

    Both still precede the gate step, so a position check that only compares
    each checkout with the gate accepts them. On a fresh runner the candidate
    checkout then prepares the workspace root and removes the verifier's
    directory, so the gate loses its script.
    """
    steps = job_steps(release, ADMISSION_JOB)
    candidate = _checkout_with_path(release, has_path=False)
    verifier = _checkout_with_path(release, has_path=True)
    first, second = steps.index(candidate), steps.index(verifier)
    steps[first], steps[second] = steps[second], steps[first]


def _workflow_default_moves_gate(release: dict[str, object]) -> None:
    """Move every run step of the whole workflow into the verifier checkout.

    The job and the step set no working directory, so a reader that checks only
    those two sees nothing wrong. A workflow-level default applies to every job
    all the same, and this gate would then resolve its script below
    ``<path>/<path>/``.

    Raises
    ------
    MutationError
        If the verifier checkout declares no path, so there is no directory to
        move the workflow into.
    """
    declared = _checkout_inputs(release, has_path=True)
    path = declared.get("path")
    if not isinstance(path, str) or not path:
        raise MutationError(MutationIssue.PATTERN_MISMATCH, "path")
    release["defaults"] = {"run": {"working-directory": path}}


def _gate_run_splits_the_command(release: dict[str, object]) -> None:
    """Break the gate command at a shell command boundary.

    This is the shape a literal ``|-`` scalar produces: the newline before
    ``--script`` is preserved, so Bash runs ``uv run --no-config
    --no-project`` and then looks for a program called ``--script``. A
    comparison that folds the newline into a space accepts the command as
    written.

    Raises
    ------
    MutationError
        If the command does not contain exactly one ` --script ` boundary, so
        the split the mutation describes is not the command's shape.
    """
    step = _gate_step(release)
    run = str(step.get("run", ""))
    marker = " --script "
    if run.count(marker) != 1:
        raise MutationError(MutationIssue.PATTERN_MISMATCH, marker)
    step["run"] = run.replace(marker, "\n--script ")


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


def _gate_without_uv_isolation(release: dict[str, object]) -> None:
    """Run uv over the candidate checkout's configuration and project files.

    The gate keeps the candidate checkout as its working directory, and uv
    reads `uv.toml` and `pyproject.toml` from there and its parents. Without
    the isolation flags a candidate can name the index the gate's own pinned
    dependencies come from, and the code those dependencies run has the job's
    `GH_TOKEN`.

    Raises
    ------
    MutationError
        If the gate step no longer carries the flags as one adjacent pair. The
        match is by pattern rather than by literal text so a reflowed command
        is still mutated, and exactly one span is required so a command that
        carries the pair twice is not silently left half-isolated.
    """
    step = _gate_step(release)
    run = str(step.get("run", ""))
    stripped, removals = _UV_ISOLATION_SPAN.subn("", run)
    if removals != 1:
        raise MutationError(MutationIssue.PATTERN_MISMATCH, UV_ISOLATION_FLAGS)
    step["run"] = stripped


#: Each mutation that breaks the candidate/verifier split, keyed by its label.
CHECKOUT_MUTATIONS: dict[str, cabc.Callable[[dict[str, object]], None]] = {
    "caller-selects-gate-implementation": _caller_selects_gate,
    "gate-from-caller-repository": _gate_from_caller_repository,
    "gate-from-run-commit-only": _gate_from_run_commit_only,
    "single-pinned-checkout": _single_pinned_checkout,
    "gate-checkout-replaces-candidate": _gate_checkout_replaces_candidate,
    "candidate-checkout-names-verifier": _candidate_checkout_names_verifier,
    "candidate-checkout-conditional": _candidate_checkout_conditional,
    "candidate-checkout-tolerates-failure": _candidate_checkout_tolerates_failure,
    "verifier-checkout-conditional": _verifier_checkout_conditional,
    "verifier-checkout-tolerates-failure": _verifier_checkout_tolerates_failure,
    "verifier-persists-credentials": _verifier_persists_credentials,
    "verifier-path-needs-quoting": _verifier_path_needs_quoting,
    "checkouts-swapped": _checkouts_swapped,
    "gate-runs-candidate-script": _gate_runs_candidate_script,
    "gate-step-leaves-candidate": _gate_step_leaves_candidate,
    "job-default-moves-gate": _job_default_moves_gate,
    "workflow-default-moves-gate": _workflow_default_moves_gate,
    "gate-run-splits-the-command": _gate_run_splits_the_command,
    "gate-run-without-uv-isolation": _gate_without_uv_isolation,
}

__all__ = ("CHECKOUT_MUTATIONS",)
