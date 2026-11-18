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
working directory alone. What each checkout must name, and the subdirectory
rule, live in :mod:`release_admission_checkout_inputs`; this module composes
them with the order and gate-step rules. The enforcement mode that the write
jobs consume is checked separately in :mod:`release_admission_gate_contract`.

Run via ``make test-workflow-contracts``.
"""

from release_admission_checkout_inputs import (
    CANDIDATE_REF,
    CANDIDATE_REPOSITORY,
    GATE_REF,
    GATE_REPOSITORY,
    Report,
    check_candidate_inputs,
    check_verifier_inputs,
)
from workflow_loading import job_steps, workflow_job

ADMISSION_JOB = "release-admission-canaries"
ADMISSION_STEP_ID = "release_admission"
CHECKOUT_ACTION = "actions/checkout"
#: The gate's entry point, relative to the root of the checkout that supplies
#: it. The gate step must name the pinned verifier's copy of this path.
GATE_SCRIPT = ".github/scripts/release_admission.py"

#: The flags that keep uv's configuration and project discovery out of the
#: candidate checkout. The gate has to run from the workspace root so the
#: candidate fetch keeps the caller's `origin`, and that root is untrusted
#: content: uv reads `uv.toml` and `pyproject.toml` from the working directory
#: and its parents, so without these a candidate could redirect the package
#: index that supplies the gate's own pinned dependencies and run its code
#: inside the admission job. `--no-config` refuses the configuration files and
#: `--no-project` refuses project discovery; neither suppresses the script's
#: own PEP 723 dependencies, which uv still resolves and installs.
UV_ISOLATION_FLAGS = "--no-config --no-project"


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
    candidate_report = Report(f"{label}.candidate", violations)
    candidate = _role_checkout(
        steps, checkouts, has_path=False, report=candidate_report
    )
    verifier_report = Report(f"{label}.verifier", violations)
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
    check_candidate_inputs(steps[candidate], candidate_report)
    gate_path = check_verifier_inputs(steps[verifier], verifier_report)
    _check_gate_step(
        release,
        steps[gate_index],
        gate_path,
        Report(f"{label}.gate", violations),
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
    report: Report,
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


def _gate_command(gate_path: str) -> str:
    """Return the command that runs the gate script from ``gate_path``.

    The isolation flags are part of the command rather than an optional
    addition: the working directory this command keeps is the untrusted
    candidate checkout, so a gate run without them would let that checkout
    decide which packages the verifier imports.

    Returns
    -------
    str
        The uv invocation, naming the verifier's copy of the entry point.
    """
    return f"uv run {UV_ISOLATION_FLAGS} --script {gate_path}/{GATE_SCRIPT}"


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
    report: Report,
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
    report: Report,
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
    "UV_ISOLATION_FLAGS",
    "check_admission_source_pin",
    "verifier_gate_command",
)
