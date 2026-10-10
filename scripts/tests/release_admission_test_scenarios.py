"""Answer the controlled failures the release-admission matrices exercise.

The runtime modules drive the real gate through cmd-mox doubles. This module
owns the other half of that arrangement: the wrappers that make one documented
boundary answer something other than its happy path, and the case record that
names those wrappers alongside the classification each one must produce.

Every wrapper is composed over the handler already installed rather than
replacing it, so a case states only the one thing it changes. That is what
keeps a case readable as a statement about a boundary -- "the commit query
answers a different revision" -- instead of a restatement of the happy path the
case is not testing.

The wrappers here are the port of the retired executable fakes. Where a fake
read a ``NETSUKE_FAKE_*`` environment variable to decide what to do, a wrapper
decides it in the test process, which removes the environment variable, the
shell script, and the call log from the picture entirely.

A shim cannot be made to ignore ``SIGTERM`` -- it is a fixed script the test
does not own, so the terminate-and-kill escalation reaches it as an ordinary
death. The escalation is therefore exercised against the bounded-run helper
directly, with a real child, in
:mod:`test_release_admission_metric_boundedness`.

Deferred refusals
-----------------
RFC 0005 names evidence refusals this suite cannot reach, because the gate has
no producer of its own yet. The gate reads the freshness state from
``NETSUKE_RELEASE_ADMISSION_EVIDENCE_STATE`` and the run identifier from the
API, so a missing or malformed *evidence manifest*, a *digest mismatch* between
an archive and its sidecar, a failing *advisory or licence* scan, and a
*performance budget* breach are all refusals the evidence producer must raise.
None is reachable from the gate's own inputs, and each is therefore recorded
here as deferred rather than tested:

- a missing or malformed evidence manifest,
- an archive whose digest disagrees with its sidecar,
- a failing advisory or licence scan, and
- a breached performance budget.

They become reachable when the producer lands. The gate's own side of that
contract -- the ``verify_evidence`` operation and its ``missing_evidence``
category -- already runs for every revision, and is covered.
"""

import dataclasses
import itertools
import time
import typing as typ

from release_admission_test_doubles import (
    Boundaries,
    Handler,
    Subprocess,
    Wrapper,
    install_boundaries,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from cmd_mox import CmdMox
    from cmd_mox.ipc import Invocation

#: The exit status a command reports when it fails on its own terms.
FAILURE_STATUS = 1

#: The revision a mismatch case answers with. It must differ from the revision
#: the workflow exported, which is what makes the classification a mismatch.
MISMATCHED_REVISION = "b" * 40


def is_commit_query(invocation: Invocation) -> bool:
    """Return whether an invocation asks the API to resolve a commit."""
    return any("/commits/" in argument for argument in invocation.args)


def is_workflow_run_query(invocation: Invocation) -> bool:
    """Return whether an invocation asks the API for a workflow run."""
    return any("/actions/runs?" in argument for argument in invocation.args)


def _gated(predicate: cabc.Callable[[Invocation], bool], answer: Handler) -> Wrapper:
    """Return a wrapper that answers only the invocations *predicate* selects.

    Every wrapper in this module has the same shape -- decide whether this
    invocation is the one being controlled, answer it if so, and delegate it to
    the happy path if not -- so the shape lives here once and each wrapper
    supplies only its predicate and its answer.

    Returns
    -------
    Wrapper
        A wrapper that substitutes *answer* for the selected invocations.
    """

    def wrapper(invocation: Invocation, handler: Handler) -> tuple[str, str, int]:
        """Answer through the wrapped handler, or with the substitution."""
        return answer(invocation) if predicate(invocation) else handler(invocation)

    return wrapper


def _refusing(message: str) -> Handler:
    """Return a handler that fails every invocation with *message* on stderr."""

    def answer(_invocation: Invocation) -> tuple[str, str, int]:
        """Refuse with the fixed message and a failing status."""
        return "", message, FAILURE_STATUS

    return answer


def _answering(stdout: str) -> Handler:
    """Return a handler that succeeds with *stdout*."""

    def answer(_invocation: Invocation) -> tuple[str, str, int]:
        """Succeed with the fixed standard output."""
        return stdout, "", 0

    return answer


def _delayed(seconds: float, handler: Handler) -> Handler:
    """Return *handler* delayed by *seconds*.

    Returns
    -------
    Handler
        A handler that waits and then answers through *handler*.

    Notes
    -----
    A delay must not reach the shim's own client timeout of five seconds. Past
    it the shim reports a transport failure rather than the answer, so a case
    would be measuring cmd-mox instead of the gate's own bound.
    """

    def answer(invocation: Invocation) -> tuple[str, str, int]:
        """Wait, then answer through the wrapped handler."""
        time.sleep(seconds)
        return handler(invocation)

    return answer


def fail_commit_query() -> Wrapper:
    """Return a wrapper that fails the commit-resolution query only.

    The default GitHub double fails for no query at all, so this is how a case
    reaches ``api_error`` on ``resolve_tag_commit`` while leaving the later run
    query answering normally.

    Returns
    -------
    Wrapper
        A wrapper that refuses the commit query and delegates the rest.
    """
    return _gated(is_commit_query, _refusing("the commit lookup failed\n"))


def fail_workflow_run_query() -> Wrapper:
    """Return a wrapper that fails the workflow-run query only.

    Returns
    -------
    Wrapper
        A wrapper that refuses the run query and delegates the rest.
    """
    return _gated(is_workflow_run_query, _refusing("the run lookup failed\n"))


def answer_other_commit(revision: str) -> Wrapper:
    """Return a wrapper that resolves a commit to *revision*.

    Parameters
    ----------
    revision
        The SHA the commit query answers, which must differ from the revision
        the workflow exported for the gate to classify a mismatch.

    Returns
    -------
    Wrapper
        A wrapper that rewrites the commit answer and delegates the rest.
    """
    return _gated(is_commit_query, _answering(f"{revision}\n"))


def answer_no_workflow_run() -> Wrapper:
    """Return a wrapper that reports no workflow run for the revision.

    The API filter ``// empty`` prints an empty line rather than ``null`` when a
    revision has no run, and an empty identifier is what ``classify_evidence``
    refuses once the freshness check has succeeded.

    Returns
    -------
    Wrapper
        A wrapper that empties the run answer and delegates the rest.
    """
    return _gated(is_workflow_run_query, _answering("\n"))


def delay_commit_query(seconds: float) -> Wrapper:
    """Return a wrapper that delays only the commit-resolution query.

    Returns
    -------
    Wrapper
        A wrapper that makes ``resolve_tag_commit`` outlive its bound.
    """
    return _gated(is_commit_query, _delayed(seconds, _answering("\n")))


def delay_workflow_run_query(seconds: float) -> Wrapper:
    """Return a wrapper that delays only the workflow-run query.

    Returns
    -------
    Wrapper
        A wrapper that makes ``fetch_workflow_run`` outlive its bound.
    """
    return _gated(is_workflow_run_query, _delayed(seconds, _answering("\n")))


def fail_git(_invocation: Invocation, _handler: Handler) -> tuple[str, str, int]:
    """Refuse the bounded fetch, as a fetch that cannot reach the remote does.

    Returns
    -------
    tuple[str, str, int]
        Standard output, standard error, and the fetch failure status.
    """
    return "", "the fetch failed\n", FAILURE_STATUS


def fail_nth_call(number: int, message: str) -> Wrapper:
    """Return a wrapper that refuses the *number*-th invocation only.

    A sink or a clock that fails once and then recovers is how the shell's own
    asymmetry is observed: a refused trace write sets a flag the gate reports
    later, while a refused clock read costs one operation's result. Neither is
    reachable by failing every call, because a persistently broken boundary
    stops the chain before the recovery the record is about.

    Returns
    -------
    Wrapper
        A wrapper that refuses one numbered invocation and delegates the rest.
    """
    remaining = itertools.count(1)

    def wrapper(invocation: Invocation, handler: Handler) -> tuple[str, str, int]:
        """Refuse the selected invocation, and answer every other normally."""
        if next(remaining) == number:
            return "", message, FAILURE_STATUS
        return handler(invocation)

    return wrapper


@dataclasses.dataclass(frozen=True, slots=True)
class FailureCase:
    """Describe one controlled admission failure and its bounded category.

    Attributes
    ----------
    evidence_state
        Evidence state supplied to the admission subprocess.
    operation
        Operation expected to classify the failure.
    error_category
        Bounded category expected for the failure.
    adapters
        Wrappers applied to named boundaries, keyed by a ``Boundaries`` field.
    environment
        Literal child-environment overrides the case needs.
    enforce
        Whether the subprocess runs in enforcement mode.
    run_lookup_succeeds
        Whether the workflow-run lookup itself succeeds. A case that empties
        the run identifier sets this, because the lookup has to reach the
        evidence check for the empty identifier to be what refuses it.

    Notes
    -----
    Contract invariants: operation and error category are fixed vocabulary
    members; every wrapper is composed over the boundary's happy path, and a
    case that names no wrapper runs the documented defaults unchanged.
    """

    evidence_state: str
    operation: str
    error_category: str
    adapters: dict[str, Wrapper] = dataclasses.field(default_factory=dict)
    environment: dict[str, str] = dataclasses.field(default_factory=dict)
    enforce: bool = True
    run_lookup_succeeds: bool = False

    def environment_for(self) -> dict[str, str]:
        """Return the child environment this case adds to the harness one.

        Returns
        -------
        dict[str, str]
            The enforcement mode plus every literal override.
        """
        return {
            "NETSUKE_RELEASE_ADMISSION_ENFORCE": str(self.enforce).lower(),
            **self.environment,
        }

    def apply(self, boundaries: Boundaries) -> Boundaries:
        """Compose this case's wrappers over the boundaries it names.

        Parameters
        ----------
        boundaries
            Doubles that already answer their happy paths.

        Returns
        -------
        Boundaries
            The same doubles, with this case's wrappers composed over them.
        """
        for name, wrapper in self.adapters.items():
            double: Subprocess = getattr(boundaries, name)
            double.wraps(wrapper)
        return boundaries


def install_case(cmd_mox: CmdMox, case: FailureCase) -> Boundaries:
    """Install the default doubles and apply one case's wrappers.

    Parameters
    ----------
    cmd_mox
        Active controller that owns the shims.
    case
        The failure case whose wrappers must be composed.

    Returns
    -------
    Boundaries
        The three registered doubles with the case applied.
    """
    return case.apply(install_boundaries(cmd_mox))


__all__ = (
    "FAILURE_STATUS",
    "MISMATCHED_REVISION",
    "FailureCase",
    "answer_no_workflow_run",
    "answer_other_commit",
    "delay_commit_query",
    "delay_workflow_run_query",
    "fail_commit_query",
    "fail_git",
    "fail_nth_call",
    "fail_workflow_run_query",
    "install_case",
    "is_commit_query",
    "is_workflow_run_query",
)
