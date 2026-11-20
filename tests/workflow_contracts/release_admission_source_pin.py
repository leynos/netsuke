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

The same workspace root decides more than the gate's command. `setup-uv`
defaults its `working-directory` to the workspace root and, when no `version`
input is set, resolves the uv release from a `uv.toml` or `pyproject.toml` it
finds there -- files the candidate supplies. The release workflow therefore
names the uv release on every setup step, and this module requires it, so the
tool that runs the gate cannot be chosen by the content the gate judges.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_admission_checkout_inputs import (
    CANDIDATE_REF,
    CANDIDATE_REPOSITORY,
    GATE_REF,
    GATE_REPOSITORY,
    Report,
    check_candidate_inputs,
    check_verifier_inputs,
    normalise_gate_path,
)
from workflow_loading import job_steps, require_mapping, workflow_job

if typ.TYPE_CHECKING:
    import collections.abc as cabc

ADMISSION_JOB = "release-admission-canaries"
ADMISSION_STEP_ID = "release_admission"
CHECKOUT_ACTION = "actions/checkout"
#: The action that installs uv. Its `version` input is the release it installs;
#: left unset, it resolves one from the working directory's project files.
SETUP_UV_ACTION = "astral-sh/setup-uv"
#: The uv release every setup step installs. A pinned action commit fixes the
#: installer's code, not the uv binary it downloads: without this input the
#: release is whatever the action resolves at run time, and a candidate's
#: `uv.toml` or `pyproject.toml` at the workspace root is one of the inputs it
#: resolves from. The value is the release the repository's own Python gates
#: run, so a local pass and a CI pass use the same uv.
PINNED_UV_VERSION = "0.13.2"
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

    The tool that runs the gate is part of that boundary, so every `setup-uv`
    step in the workflow must name the uv release it installs rather than let
    the action resolve one from project files the candidate supplies.

    Parameters
    ----------
    release
        Parsed release workflow.
    violations
        Collector for the job's findings.
    """
    _check_setup_uv_pins(release, violations)
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
    if candidate > verifier:
        violations.append(
            f"{label}.candidate: the candidate checkout must precede the "
            "verifier checkout, because the candidate prepares the workspace "
            "root and removes the verifier's directory when it is written first"
        )
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


def _check_setup_uv_pins(release: dict[str, object], violations: list[str]) -> None:
    """Require every uv setup step to name the release it installs.

    `setup-uv` resolves an unset `version` from its working directory: its
    default is the workspace root, where the candidate checkout sits, and it
    reads `uv.toml` and `pyproject.toml` from there. Candidate content could
    therefore choose the uv binary that runs a gate holding the job's token.
    The check covers every job rather than the admission job alone because the
    same checkout shape and the same resolution order decide each one.

    Parameters
    ----------
    release
        Parsed release workflow.
    violations
        Collector for the findings.
    """
    for name, step in _setup_uv_steps(release):
        declared = step.get("with")
        version = declared.get("version") if isinstance(declared, dict) else None
        if version != PINNED_UV_VERSION:
            violations.append(
                f"{name}.setup-uv: must name version {PINNED_UV_VERSION!r} "
                f"so candidate project files cannot choose the uv release, "
                f"got {version!r}"
            )


def _setup_uv_steps(
    release: dict[str, object],
) -> cabc.Iterator[tuple[str, dict[str, object]]]:
    """Yield every job's uv setup steps, labelled with the job's name.

    A job that calls a reusable workflow declares no steps of its own, so it
    contributes nothing here.

    Yields
    ------
    tuple[str, dict[str, object]]
        Each job's name and one of its ``setup-uv`` steps, in declaration order.
    """
    jobs = require_mapping(release.get("jobs"), "release.jobs")
    for name, job in jobs.items():
        for step in _mapping_steps(job):
            if str(step.get("uses", "")).startswith(f"{SETUP_UV_ACTION}@"):
                yield str(name), step


def _mapping_steps(job: object) -> list[dict[str, object]]:
    """Return the step mappings a job declares, and none for a job without."""
    steps = job.get("steps") if isinstance(job, dict) else None
    if not isinstance(steps, list):
        return []
    return [step for step in steps if isinstance(step, dict)]


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
    return _gate_command(normalise_gate_path(str(declared["path"])))


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
        # Only the outer whitespace a YAML block scalar leaves behind is
        # trimmed. An internal newline is a shell command boundary, so a `|-`
        # scalar that splits `--script` onto its own line runs something else
        # entirely; normalising it to a space would report that broken command
        # as the expected one.
        run = str(step.get("run", "")).strip()
        if run != expected:
            report.add(f"the gate must run {expected!r}, got {run!r}")
    _check_working_directory(release, step, report)


def _check_working_directory(
    release: dict[str, object],
    step: dict[str, object],
    report: Report,
) -> None:
    """Require the gate step to keep the candidate checkout as its directory.

    Three settings can move a step, and the narrower one wins: the step's own
    ``working-directory``, the job's ``defaults.run``, and the workflow's
    ``defaults.run`` at the document root. A workflow-level default applies to
    every job, so a value the job never repeats still moves this gate -- and
    with the verifier's directory it would resolve the script below
    ``<path>/<path>/`` instead.
    """
    declared = step.get("working-directory")
    if declared is not None:
        report.add(
            "the gate must keep the candidate checkout as its working "
            f"directory, got {declared!r}"
        )
    _check_default_working_directory(release.get("defaults"), "workflow", report)
    _check_default_working_directory(
        workflow_job(release, ADMISSION_JOB).get("defaults"), "job", report
    )


def _check_default_working_directory(
    defaults: object, where: str, report: Report
) -> None:
    """Report a ``defaults.run.working-directory`` that would move the gate."""
    run_defaults = defaults.get("run") if isinstance(defaults, dict) else None
    if isinstance(run_defaults, dict) and "working-directory" in run_defaults:
        report.add(
            f"the {where} must not set a default run working directory, which "
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
    "PINNED_UV_VERSION",
    "SETUP_UV_ACTION",
    "UV_ISOLATION_FLAGS",
    "check_admission_source_pin",
    "verifier_gate_command",
)
