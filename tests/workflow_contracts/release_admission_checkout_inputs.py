"""State what each admission checkout must name, and hold it to that.

The admission job checks out two commits, and each one has to name a specific
repository and ref: the candidate names the caller's own repository and commit,
so the gate's fetch reaches the revision under judgement, and the verifier names
this repository at the commit that defines the job, so the gate's implementation
cannot come from the caller. A checkout that names something plausible but
different -- the caller instead of this repository, or the run's commit instead
of the defining commit -- leaves a workflow that still reads as correct, which
is why each expectation is stated here with the reason it exists.

This module owns those literals and the checks that hold a checkout to them,
including the rule that the verifier's ``path`` stays a subdirectory below the
workspace root. :func:`check_admission_source_pin` in
:mod:`release_admission_source_pin` composes them with the order and gate-step
rules.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import re
import typing as typ
from pathlib import PurePosixPath

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
class Report:
    """Collect one role's findings under the label they are reported for.

    Every rule reports against a single role -- the candidate checkout, the
    verifier checkout, or the gate step -- so the role label and the collector
    travel together rather than being repeated at each call.
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

#: The keys a checkout must not declare at all. Both accept expressions, so no
#: declared value can be judged: `if: false` skips the verifier while the gate
#: step still runs, and `continue-on-error: true` lets a failed checkout leave a
#: partial workspace the gate then reads. Either way the gate would run whatever
#: the candidate checkout holds at its own script path.
_FORBIDDEN_CHECKOUT_KEYS = ("if", "continue-on-error")

#: The input that keeps the verifier checkout's credential out of the workspace
#: the candidate's content occupies. Only the verifier is held to it: the
#: candidate checkout keeps its default so the gate can fetch a private
#: caller's repository, and nothing from that checkout is executed.
_REQUIRES_NO_PERSISTED_CREDENTIALS = "persist-credentials"

#: The characters a checkout path's components may use. Each component reaches
#: the gate command as one shell word, so a component the shell would split,
#: expand, or quote is refused: `gate files` would arrive as two arguments.
_SHELL_SAFE_COMPONENT = re.compile(r"[A-Za-z0-9._-]+")


def _inputs(step: dict[str, object], report: Report) -> dict[str, object] | None:
    """Return the checkout's declared inputs, reporting a malformed shape."""
    declared = step.get("with")
    if not isinstance(declared, dict):
        report.add("the checkout must declare its inputs")
        return None
    return declared


def _check_input(
    declared: dict[str, object], requirement: _ExpectedInput, report: Report
) -> None:
    """Require one checkout input to resolve to the value the role needs."""
    actual = " ".join(str(declared.get(requirement.key, "")).split())
    if actual != requirement.expected:
        report.add(
            f"{requirement.key} must be {requirement.expected!r} "
            f"{requirement.rationale}, got {declared.get(requirement.key)!r}"
        )


def check_candidate_inputs(step: dict[str, object], report: Report) -> None:
    """Require the candidate checkout to select the caller's own commit.

    Parameters
    ----------
    step
        The one checkout declared at the workspace root.
    report
        Collector the findings are attributed to.
    """
    _check_unconditional(step, report)
    declared = _inputs(step, report)
    if declared is None:
        return
    _check_input(declared, _REQUIRES_CALLER_REPOSITORY, report)
    _check_input(declared, _REQUIRES_CALLER_COMMIT, report)


def check_verifier_inputs(step: dict[str, object], report: Report) -> str | None:
    """Require the verifier checkout to select this job's defining commit.

    Parameters
    ----------
    step
        The one checkout that declares a ``path`` input.
    report
        Collector the findings are attributed to. A returned path does not
        mean the checkout passed: a wrong repository or ref is recorded here
        rather than raised, so the caller reads ``report`` to decide.

    Returns
    -------
    str | None
        The verifier's checkout path, which the gate step must run its script
        from, or ``None`` when the path itself is unusable.
    """
    _check_unconditional(step, report)
    declared = _inputs(step, report)
    if declared is None:
        return None
    _check_input(declared, _REQUIRES_VERIFIER_REPOSITORY, report)
    _check_input(declared, _REQUIRES_WORKFLOW_COMMIT, report)
    _check_no_persisted_credentials(declared, report)
    return _check_gate_path(declared, report)


def _check_unconditional(step: dict[str, object], report: Report) -> None:
    """Require a checkout the job runs unconditionally and cannot tolerate failing.

    A skipped or tolerated checkout leaves the gate step with no verifier, and
    the gate then runs the script path from whatever the candidate checkout
    holds there -- which is candidate-supplied code with the job's token.
    """
    for key in _FORBIDDEN_CHECKOUT_KEYS:
        if key in step:
            report.add(
                f"the checkout must not declare {key!r}, which can skip it or "
                "tolerate its failure and leave the gate running the "
                "candidate's own script"
            )


def _check_no_persisted_credentials(
    declared: dict[str, object], report: Report
) -> None:
    """Require the verifier checkout to leave its credential out of the workspace."""
    actual = declared.get(_REQUIRES_NO_PERSISTED_CREDENTIALS)
    if actual is not False:
        report.add(
            f"{_REQUIRES_NO_PERSISTED_CREDENTIALS} must be false so the "
            f"verifier's credential is not left beside the candidate's content, "
            f"got {actual!r}"
        )


def normalise_gate_path(path: str) -> str:
    """Return a declared checkout path in the form every contract compares.

    One validator reads the path from the checkout inputs and another rebuilds
    the expected gate command from the same declaration. Comparing the raw
    spelling in one place and the normalised spelling in the other makes
    ``./gate`` fail both, so both go through this function.

    Parameters
    ----------
    path
        The declared ``path`` input, as the workflow spells it.

    Returns
    -------
    str
        The path with surrounding whitespace removed and ``.`` components
        dropped, so ``./gate`` and ``gate`` compare equal.
    """
    return str(PurePosixPath(path.strip()))


def _check_gate_path(declared: dict[str, object], report: Report) -> str | None:
    """Require a verifier checkout path that stays inside the workspace.

    The path reaches the gate command as one unquoted shell word, so it must
    also be one: a directory named ``gate files`` would be split into two
    arguments and the gate would not run at all.

    Returns
    -------
    str | None
        The normalised relative path, or ``None`` when the declared value is
        absent, absolute, escapes the workspace with ``..``, or names a
        component the shell would not take literally.
    """
    raw = declared.get("path")
    if not isinstance(raw, str) or not raw.strip():
        report.add(
            f"path must name a subdirectory below the workspace root, got {raw!r}"
        )
        return None
    path = PurePosixPath(normalise_gate_path(raw))
    if _escapes_workspace(path):
        report.add(
            "path must be a relative subdirectory that cannot replace the "
            f"candidate checkout, got {raw!r}"
        )
        return None
    if not all(_SHELL_SAFE_COMPONENT.fullmatch(part) for part in path.parts):
        report.add(
            "path must name components the gate command can pass as one shell "
            f"argument each, got {raw!r}"
        )
        return None
    return str(path)


def _escapes_workspace(path: PurePosixPath) -> bool:
    """Return whether a checkout path could write outside its subdirectory."""
    return path.is_absolute() or ".." in path.parts or str(path) == "."


__all__ = (
    "CANDIDATE_REF",
    "CANDIDATE_REPOSITORY",
    "GATE_REF",
    "GATE_REPOSITORY",
    "Report",
    "check_candidate_inputs",
    "check_verifier_inputs",
    "normalise_gate_path",
)
