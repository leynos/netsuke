"""Require the admission job's two checkouts to stay in their own roles.

An admission gate for a reusable workflow has to answer two different
questions from two different commits. The candidate -- the revision whose
evidence is being judged -- has to be fetchable, and it lives in the *caller's*
repository. The verifier -- the code that decides whether the candidate is
admitted -- has to be this repository's own, at the commit that defines the
job, or a caller could supply a gate that reports whatever the write jobs want
to hear.

One checkout cannot serve both. A checkout at the workspace root leaves
`origin` naming the repository it checked out, and the gate fetches the
candidate from `origin`, so pinning that checkout at the verifier re-points
`origin` away from the caller. The job therefore checks out the candidate at
the workspace root and the verifier in a subdirectory, and runs the verifier's
script from the workspace root so `origin` keeps naming the caller.

:func:`check_admission_source_pin` holds that shape: two checkouts, each with
its own repository and ref, the verifier confined to a relative subdirectory,
and a gate step whose command names the verifier's script while leaving the
working directory alone. The enforcement mode that the write jobs consume is
checked separately in :mod:`release_admission_gate_contract`.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import typing as typ
from pathlib import PurePosixPath

from workflow_loading import job_steps, workflow_job

ADMISSION_JOB = "release-admission-canaries"
ADMISSION_STEP_ID = "release_admission"
CHECKOUT_ACTION = "actions/checkout"
#: The gate's entry point, relative to the root of the checkout that supplies
#: it. The gate step must name the pinned verifier's copy of this path.
GATE_SCRIPT = ".github/scripts/release_admission.py"

#: The candidate checkout names the caller's own repository and commit. It is
#: what leaves `origin` pointing at the repository the candidate belongs to, so
#: the gate's fetch of the candidate revision reaches a commit this repository
#: may not contain; nothing in it is executed.
CANDIDATE_REPOSITORY = "${{ github.repository }}"
CANDIDATE_REF = "${{ github.sha }}"

#: The verifier checkout names the repository whose commit defines the gate
#: implementation. It must not be the caller's: a reusable workflow's default
#: checkout resolves the caller, so an unpinned job runs whatever gate the
#: caller's candidate contains.
GATE_REPOSITORY = "leynos/netsuke"
#: The ref that names the workflow's own commit. `job.workflow_sha` is the
#: commit of the workflow file that defines this job, which is a commit of this
#: repository both for a reusable-workflow call and for a directly triggered
#: run. A fallback to `github.sha` would not do: on a call it is the caller's
#: candidate, which is the commit the pin exists to refuse.
GATE_REF = "${{ job.workflow_sha }}"


@dataclasses.dataclass(frozen=True, slots=True)
class _Report:
    """Collect one role's findings under the label they are reported for.

    Every rule below reports against a single role -- the candidate checkout,
    the verifier checkout, or the gate step -- so the role label and the
    collector travel together rather than being repeated at each call.
    """

    label: str
    violations: list[str]

    def add(self, message: str) -> None:
        """Record one finding, attributed to this report's role."""
        self.violations.append(f"{self.label}: {message}")


class _ExpectedInput(typ.NamedTuple):
    """Name one checkout input and the value the role requires of it.

    The rationale travels with the expectation because it is what tells an
    operator *why* the value is required.
    """

    key: str
    expected: str
    rationale: str


_REQUIRES_CALLER_REPOSITORY = _ExpectedInput(
    "repository",
    CANDIDATE_REPOSITORY,
    "so `origin` is the repository the candidate belongs to",
)
_REQUIRES_CALLER_COMMIT = _ExpectedInput(
    "ref",
    CANDIDATE_REF,
    "so the checkout is this run's candidate",
)
_REQUIRES_VERIFIER_REPOSITORY = _ExpectedInput(
    "repository",
    GATE_REPOSITORY,
    "so the gate is this repository's, not the caller's",
)
_REQUIRES_WORKFLOW_COMMIT = _ExpectedInput(
    "ref",
    GATE_REF,
    "so the gate comes from the workflow's own commit",
)


def check_admission_source_pin(
    release: dict[str, object], violations: list[str]
) -> None:
    """Require the two checkouts that separate the candidate from the verifier.

    The job needs two commits that must not be confused. The candidate checkout
    is what leaves `origin` pointing at the caller's repository, so the gate's
    fetch of the candidate revision can reach a commit this repository does not
    have. The verifier checkout is the gate's own source, pinned to the commit
    that defines this job, so the mode, outcome and candidate the write jobs
    trust cannot come from the caller. The gate step must then run the
    verifier's script while keeping the candidate checkout as its working
    directory: running it from the verifier would fetch the candidate from this
    repository instead.

    Parameters
    ----------
    release
        Parsed release workflow.
    violations
        Collector for the job's findings.
    """
    steps = job_steps(release, ADMISSION_JOB)
    label = f"{ADMISSION_JOB}.checkout"
    checkouts = _checkout_indexes(steps)
    if len(checkouts) != 2:
        violations.append(
            f"{label}: expected two {CHECKOUT_ACTION} steps -- the candidate and "
            f"the pinned verifier -- found {len(checkouts)}"
        )
        return
    candidate_report = _Report(f"{label}.candidate", violations)
    candidate = _role_checkout(
        steps, checkouts, has_path=False, report=candidate_report
    )
    verifier_report = _Report(f"{label}.verifier", violations)
    verifier = _role_checkout(steps, checkouts, has_path=True, report=verifier_report)
    if candidate is None or verifier is None:
        return
    gate_index = _gate_index(steps, violations)
    if gate_index is None:
        return
    for index, role in ((candidate, "candidate"), (verifier, "verifier")):
        if index > gate_index:
            violations.append(
                f"{label}.{role}: the checkout must precede the "
                f"{ADMISSION_STEP_ID!r} step"
            )
    _check_candidate_inputs(steps[candidate], candidate_report)
    gate_path = _check_verifier_inputs(steps[verifier], verifier_report)
    _check_gate_step(
        release,
        steps[gate_index],
        gate_path,
        _Report(f"{label}.gate", violations),
    )


def _checkout_indexes(steps: list[dict[str, object]]) -> list[int]:
    """Return the indexes of every checkout step, in declaration order."""
    return [
        index
        for index, step in enumerate(steps)
        if str(step.get("uses", "")).startswith(f"{CHECKOUT_ACTION}@")
    ]


def _role_checkout(
    steps: list[dict[str, object]],
    checkouts: list[int],
    *,
    has_path: bool,
    report: _Report,
) -> int | None:
    """Return the one checkout with, or without, a ``path`` input.

    The ``path`` input is what tells the two roles apart, and it is also the
    safety property: a verifier checkout without one writes to the workspace
    root and replaces the candidate checkout, which re-points `origin` at this
    repository.

    Returns
    -------
    int | None
        The index of that checkout, or ``None`` when the workflow declares no
        single checkout of the requested shape.
    """
    matches = [index for index in checkouts if _declares_path(steps[index]) is has_path]
    if len(matches) != 1:
        placement = "with a 'path' input" if has_path else "at the workspace root"
        report.add(f"expected exactly one checkout {placement}, found {len(matches)}")
        return None
    return matches[0]


def _declares_path(step: dict[str, object]) -> bool:
    """Return whether the step's inputs include a ``path``."""
    declared = step.get("with")
    return isinstance(declared, dict) and "path" in declared


def _gate_index(steps: list[dict[str, object]], violations: list[str]) -> int | None:
    """Return the gate step's index, reporting a step that is missing."""
    index = next(
        (
            position
            for position, step in enumerate(steps)
            if step.get("id") == ADMISSION_STEP_ID
        ),
        None,
    )
    if index is None:
        violations.append(
            f"{ADMISSION_JOB}.checkout: the {ADMISSION_STEP_ID!r} step is missing"
        )
    return index


def _inputs(step: dict[str, object], report: _Report) -> dict[str, object] | None:
    """Return the checkout's declared inputs, reporting a malformed shape."""
    declared = step.get("with")
    if not isinstance(declared, dict):
        report.add("the checkout must declare its inputs")
        return None
    return declared


def _check_candidate_inputs(step: dict[str, object], report: _Report) -> None:
    """Require the candidate checkout to select the caller's own commit."""
    declared = _inputs(step, report)
    if declared is None:
        return
    _check_input(declared, _REQUIRES_CALLER_REPOSITORY, report)
    _check_input(declared, _REQUIRES_CALLER_COMMIT, report)


def _check_verifier_inputs(step: dict[str, object], report: _Report) -> str | None:
    """Require the verifier checkout to select this job's defining commit.

    Returns
    -------
    str | None
        The verifier's checkout path, which the gate step must run its script
        from, or ``None`` when the path itself is unusable.
    """
    declared = _inputs(step, report)
    if declared is None:
        return None
    _check_input(declared, _REQUIRES_VERIFIER_REPOSITORY, report)
    _check_input(declared, _REQUIRES_WORKFLOW_COMMIT, report)
    return _check_gate_path(declared, report)


def _check_gate_path(declared: dict[str, object], report: _Report) -> str | None:
    """Require a verifier checkout path that stays inside the workspace.

    Returns
    -------
    str | None
        The normalised relative path, or ``None`` when the declared value is
        absent, absolute, or escapes the workspace with ``..``.
    """
    raw = declared.get("path")
    if not isinstance(raw, str) or not raw.strip():
        report.add(
            f"path must name a subdirectory below the workspace root, got {raw!r}"
        )
        return None
    path = PurePosixPath(raw.strip())
    if _escapes_workspace(path):
        report.add(
            "path must be a relative subdirectory that cannot replace the "
            f"candidate checkout, got {raw!r}"
        )
        return None
    return str(path)


def _escapes_workspace(path: PurePosixPath) -> bool:
    """Return whether a checkout path could write outside its subdirectory."""
    return path.is_absolute() or ".." in path.parts or str(path) == "."


def _check_input(
    declared: dict[str, object], requirement: _ExpectedInput, report: _Report
) -> None:
    """Require one checkout input to resolve to the value the role needs."""
    actual = " ".join(str(declared.get(requirement.key, "")).split())
    if actual != requirement.expected:
        report.add(
            f"{requirement.key} must be {requirement.expected!r} "
            f"{requirement.rationale}, got {declared.get(requirement.key)!r}"
        )


def _gate_command(gate_path: str) -> str:
    """Return the command that runs the gate script from ``gate_path``."""
    return f"uv run --script {gate_path}/{GATE_SCRIPT}"


def verifier_gate_command(steps: list[dict[str, object]]) -> str:
    """Return the gate command implied by the job's verifier checkout.

    A contract that asserts the gate step's command without restating the
    verifier's directory name uses this, so moving the verifier checkout moves
    the expectation with it.

    Parameters
    ----------
    steps
        The admission job's steps, in declaration order.

    Returns
    -------
    str
        The command that runs the verifier's copy of the entry point.

    """
    matches = [
        step
        for step in steps
        if str(step.get("uses", "")).startswith(f"{CHECKOUT_ACTION}@")
        and _declares_path(step)
    ]
    assert len(matches) == 1, (
        "the admission job must declare exactly one verifier checkout with a "
        f"path, found {len(matches)}"
    )
    declared = matches[0]["with"]
    assert isinstance(declared, dict), (
        "the verifier checkout's inputs must be a mapping"
    )
    return _gate_command(str(declared["path"]))


def _check_gate_step(
    release: dict[str, object],
    step: dict[str, object],
    gate_path: str | None,
    report: _Report,
) -> None:
    """Require the gate step to run the verifier's script from the candidate.

    The working directory decides which repository `origin` names, so a gate
    step that runs from the verifier checkout would fetch the candidate from
    this repository and refuse a caller whose commit this repository does not
    have.
    """
    if gate_path is not None:
        expected = _gate_command(gate_path)
        run = " ".join(str(step.get("run", "")).split())
        if run != expected:
            report.add(f"the gate must run {expected!r}, got {run!r}")
    _check_working_directory(release, step, report)


def _check_working_directory(
    release: dict[str, object],
    step: dict[str, object],
    report: _Report,
) -> None:
    """Require the gate step to keep the candidate checkout as its directory."""
    declared = step.get("working-directory")
    if declared is not None:
        report.add(
            "the gate must keep the candidate checkout as its working "
            f"directory, got {declared!r}"
        )
    defaults = workflow_job(release, ADMISSION_JOB).get("defaults")
    run_defaults = defaults.get("run") if isinstance(defaults, dict) else None
    if isinstance(run_defaults, dict) and "working-directory" in run_defaults:
        report.add(
            "the job must not set a default run working directory, which "
            "would move the gate out of the candidate checkout"
        )


__all__ = (
    "ADMISSION_JOB",
    "ADMISSION_STEP_ID",
    "CANDIDATE_REF",
    "CANDIDATE_REPOSITORY",
    "CHECKOUT_ACTION",
    "GATE_REF",
    "GATE_REPOSITORY",
    "GATE_SCRIPT",
    "check_admission_source_pin",
    "verifier_gate_command",
)
