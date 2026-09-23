"""The set of workflows an event can reach, through reusable-workflow calls.

A workflow that declares only ``workflow_call`` has no trigger of its own that
a pull request intersects, yet it runs on every pull request whose workflow
calls it, and ``secrets: inherit`` hands it the caller's secrets. A contract
that enumerates workflows by trigger alone cannot see it, so every clause
built on that enumeration passes over it while it does the forbidden thing.
The pull-request lane is therefore a closure: the triggered workflows and
everything they call, transitively.

A call is local when its reference, less one of the two same-repository
prefixes GitHub documents, is a file directly under ``.github/workflows/``. The
prefixes are ``./``, which is workspace-relative, and ``$/``, the
self-repository form GitHub.com recommends. A reader knowing only one drops
callers written the other way. A cross-repository call is not
followed, because its content is not in this tree; what may be handed to one
is ``ci_coverage_wiring_invariants``'s question.

These functions read parsed documents keyed by file name, so the contract can
drive shapes the repository does not have. Run via
``make test-workflow-contracts``.
"""

import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Where GitHub looks for a same-repository reusable workflow. Reusable
#: workflows may not live in a subdirectory of it.
WORKFLOWS_PREFIX: typ.Final[str] = ".github/workflows/"

#: The prefixes GitHub documents for a same-repository call.
SELF_REPOSITORY_PREFIXES: typ.Final[tuple[str, ...]] = ("./", "$/")


class UnresolvedWorkflowCallError(LookupError):
    """Raised when a local call names a workflow the reading does not hold.

    The closure cannot vouch for a workflow it never read, so a call it cannot
    resolve fails the reading rather than dropping out of the lane.
    """


def local_workflow_name(reference: str) -> str | None:
    """Return the workflow file a same-repository call names, or None.

    Parameters
    ----------
    reference : str
        A job's ``uses:`` value.

    Returns
    -------
    str or None
        The workflow's file name when the reference, less a leading ``./`` or
        ``$/``, names a file directly under ``.github/workflows/``, otherwise
        None.

    Examples
    --------
    >>> local_workflow_name("./.github/workflows/release.yml")
    'release.yml'
    >>> local_workflow_name("$/.github/workflows/release.yml")
    'release.yml'
    >>> local_workflow_name("leynos/netsuke/.github/workflows/release.yml@main")
    """
    path = next(
        (
            reference.removeprefix(prefix)
            for prefix in SELF_REPOSITORY_PREFIXES
            if reference.startswith(prefix)
        ),
        reference,
    )
    if not path.startswith(WORKFLOWS_PREFIX):
        return None
    name = path.removeprefix(WORKFLOWS_PREFIX)
    return name if name and "/" not in name else None


def called_workflows(document: dict[str, object]) -> list[tuple[str, str]]:
    """Return the job name and reference of every reusable-workflow call.

    Parameters
    ----------
    document : dict[str, object]
        One parsed workflow document.

    Returns
    -------
    list[tuple[str, str]]
        One entry per job whose ``uses:`` is a string, in declaration order.
        Only a job calls a reusable workflow; a step's ``uses:`` names an
        action and is not read here.
    """
    jobs = document.get("jobs")
    if not isinstance(jobs, dict):
        return []
    return [
        (str(name), job["uses"])
        for name, job in jobs.items()
        if isinstance(job, dict) and isinstance(job.get("uses"), str)
    ]


def local_calls(document: dict[str, object]) -> frozenset[str]:
    """Return the file names of the same-repository workflows one calls.

    Parameters
    ----------
    document : dict[str, object]
        One parsed workflow document.

    Returns
    -------
    frozenset[str]
        Workflow file names, whether or not such a file exists.
    """
    names = (
        local_workflow_name(reference) for _, reference in called_workflows(document)
    )
    return frozenset(name for name in names if name is not None)


def reachable_workflows(
    documents: cabc.Mapping[str, dict[str, object]], entries: cabc.Iterable[str]
) -> frozenset[str]:
    """Return the entry workflows and every workflow they call, transitively.

    Parameters
    ----------
    documents : Mapping[str, dict[str, object]]
        Every workflow document, keyed by file name.
    entries : Iterable[str]
        The file names the traversal starts from, typically those whose
        triggers an event intersects.

    Returns
    -------
    frozenset[str]
        The file names reached.

    Raises
    ------
    UnresolvedWorkflowCallError
        If an entry or a local call names a file ``documents`` does not hold.
    """
    pending = list(entries)
    reached: set[str] = set()
    while pending:
        current = pending.pop()
        if current in reached:
            continue
        if current not in documents:
            message = f"{current} is called or named but was not read"
            raise UnresolvedWorkflowCallError(message)
        reached.add(current)
        pending.extend(local_calls(documents[current]) - reached)
    return frozenset(reached)
