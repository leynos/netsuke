"""Reads every coverage-invoking lane out of the workflow files.

Separated from ``timeout_budgets`` so the workflow reading and the
nextest arithmetic stay legible apart, and so neither module outgrows
the 400-line limit the Python lint gate enforces.
"""

import typing as typ

import yaml
from timeout_budgets import COVERAGE_ACTION, WATCHDOG_VARIABLE, WORKFLOWS_DIRECTORY


class CoverageLane(typ.NamedTuple):
    """One coverage step, with the budgets around it.

    Attributes
    ----------
    workflow : str
        The workflow file's name.
    job : str
        The job the step belongs to.
    step : str
        The step's declared name.
    watchdog : float or None
        The watchdog budget in seconds, or None when the job sets none
        and so inherits the action's 1,800 s default.
    job_timeout : float or None
        The job's ``timeout-minutes`` in seconds, or None when it
        declares none and so inherits GitHub's six-hour default.
    condition : tuple[object, object]
        The ``if`` on the coverage step and on its job. A skipped step
        runs no ``cargo``, so its watchdog never arms and every budget
        below says nothing about it.
    """

    workflow: str
    job: str
    step: str
    watchdog: float | None
    job_timeout: float | None
    condition: tuple[object, object] = (None, None)

    def __str__(self) -> str:
        """Return a location suitable for a failure message.

        Returns
        -------
        str
            ``workflow:job:step`` for this lane.
        """
        return f"{self.workflow}:{self.job}:{self.step!r}"


def watchdog_of(
    document: dict[str, typ.Any],
    job: dict[str, typ.Any],
    step: dict[str, typ.Any],
) -> float | None:
    """Return the watchdog budget in force for one step.

    All three levels are read, innermost first, as GitHub resolves them.
    Both workflows here set the value at job level, so a contract reading
    only the step would find nothing and report every lane as inheriting
    the action's default, which is exactly backwards. A workflow-level
    value would be missed the same way.

    Parameters
    ----------
    document : dict[str, typ.Any]
        The whole workflow document.
    job : dict[str, typ.Any]
        The enclosing job.
    step : dict[str, typ.Any]
        The coverage step.

    Returns
    -------
    float or None
        The budget in seconds, or None when no level sets one.
    """
    for owner in (step, job, document):
        environment = owner.get("env")
        raw = (
            environment.get(WATCHDOG_VARIABLE)
            if isinstance(environment, dict)
            else None
        )
        budget = _budget_from(raw)
        if budget is not None:
            return budget
    return None


class WatchdogValueError(ValueError):
    """Raised when a workflow's watchdog value cannot be read as seconds.

    Distinguished from an unset watchdog rather than folded into it. A
    lane that sets nothing inherits the action's default, which is one
    fault; a lane that sets ``abc`` has an author who meant something
    and got neither, which is another. Reporting the second as the first
    would name the wrong remedy.
    """


def _budget_from(raw: object) -> float | None:
    """Return one source's watchdog budget, or None when it sets none.

    A blank or whitespace-only value is a source that says nothing, so
    it falls through to the next one. That is what a workflow writes
    when it interpolates an expression that resolved to nothing.

    Anything else that is not a positive number of seconds is refused
    with the value in the message. The shared action reads a
    non-positive value as no timeout at all, so a lane carrying one has
    no third tier while appearing to declare one.

    Parameters
    ----------
    raw : object
        The value the workflow set, as the YAML parser returned it.

    Returns
    -------
    float or None
        The budget in seconds, or None when the source sets none.

    Raises
    ------
    WatchdogValueError
        If the value is present and non-blank but not a positive number
        of seconds.
    """
    if raw is None:
        return None
    text = str(raw).strip()
    if not text:
        return None
    try:
        seconds = float(text)
    except ValueError as error:
        message = (
            f"{WATCHDOG_VARIABLE}={raw!r} is not a number of seconds; the "
            f"lane sets a watchdog its author meant and the action will not "
            f"read"
        )
        raise WatchdogValueError(message) from error
    if seconds <= 0:
        message = (
            f"{WATCHDOG_VARIABLE}={raw!r} is not positive, so the cargo "
            f"invocation is unbounded while appearing to be bounded"
        )
        raise WatchdogValueError(message)
    return seconds


def workflow_documents() -> dict[str, dict[str, typ.Any]]:
    """Return every workflow document in the repository, keyed by name.

    This is the one place the contract touches the filesystem, so an
    unreadable or unparsable workflow fails here rather than inside a
    budget derivation. Both extensions are read. A coverage lane in the other one would
    otherwise escape every assertion below without failing anything.

    Returns
    -------
    dict[str, dict[str, typ.Any]]
        File name to parsed document.
    """
    documents: dict[str, dict[str, typ.Any]] = {}
    for pattern in ("*.yml", "*.yaml"):
        for path in sorted(WORKFLOWS_DIRECTORY.glob(pattern)):
            parsed = yaml.safe_load(path.read_text(encoding="utf-8"))
            if isinstance(parsed, dict):
                documents[path.name] = parsed
    return documents


def _mappings_in(container: object) -> list[dict[str, typ.Any]]:
    """Return the mappings in a parsed list, ignoring anything else.

    Parameters
    ----------
    container : object
        The parsed value, which need not be a list.

    Returns
    -------
    list[dict[str, typ.Any]]
        The mappings, in order.
    """
    if not isinstance(container, list):
        return []
    return [item for item in container if isinstance(item, dict)]


def _coverage_steps(job: dict[str, typ.Any]) -> list[dict[str, typ.Any]]:
    """Return the steps in one job that invoke the coverage action.

    Parameters
    ----------
    job : dict[str, typ.Any]
        The parsed job.

    Returns
    -------
    list[dict[str, typ.Any]]
        The matching steps, in the order the job runs them.
    """
    return [
        step
        for step in _mappings_in(job.get("steps"))
        if COVERAGE_ACTION in str(step.get("uses", ""))
    ]


def _lanes_in_job(
    workflow: str,
    document: dict[str, typ.Any],
    job_name: str,
    job: dict[str, typ.Any],
) -> list[CoverageLane]:
    """Return one job's coverage lanes, with the job's ceiling on each.

    Parameters
    ----------
    workflow : str
        The workflow file's name.
    document : dict[str, typ.Any]
        The enclosing document, read for a workflow-level watchdog.
    job_name : str
        The job's identifier.
    job : dict[str, typ.Any]
        The parsed job.

    Returns
    -------
    list[CoverageLane]
        One entry per coverage step in the job.

    Raises
    ------
    WatchdogValueError
        If a step's watchdog value cannot be read as a positive number
        of seconds. The lane's coordinate is added to the message, so
        the failure names the workflow and job at fault rather than
        reporting a bare conversion error.
    """
    steps = _coverage_steps(job)
    try:
        watchdogs = tuple(watchdog_of(document, job, step) for step in steps)
    except WatchdogValueError as error:
        message = f"{workflow}:{job_name}: {error}"
        raise WatchdogValueError(message) from error
    raw_timeout = job.get("timeout-minutes")
    timeout = None if raw_timeout is None else float(raw_timeout) * 60.0
    return [
        CoverageLane(
            workflow=workflow,
            job=job_name,
            step=str(step.get("name", "")) or job_name,
            watchdog=watchdogs[index],
            job_timeout=timeout,
            condition=(step.get("if"), job.get("if")),
        )
        for index, step in enumerate(steps)
    ]


def _declared_jobs(
    documents: dict[str, dict[str, typ.Any]],
) -> list[tuple[str, dict[str, typ.Any], str, dict[str, typ.Any]]]:
    """Return every job in every workflow, carrying its file and document.

    The job's identity travels with it rather than being reconstructed
    from an enclosing loop, which is what lets the lane building above be
    a single comprehension.

    Parameters
    ----------
    documents : dict[str, dict[str, typ.Any]]
        Parsed workflow documents, keyed by file name.

    Returns
    -------
    list of tuple
        Workflow name, document, job identifier, and job.
    """
    return [
        (name, document, str(job_name), job)
        for name, document in documents.items()
        for job_name, job in (document.get("jobs") or {}).items()
        if isinstance(job, dict)
    ]


def coverage_lanes_of(
    documents: dict[str, dict[str, typ.Any]] | None = None,
) -> tuple[CoverageLane, ...]:
    """Return every step invoking the coverage action, with its budgets.

    Every such step is included, not only those whose job sets a
    watchdog, so a lane that lost its override is visible as ``None``
    rather than absent. An absent entry would make the assertions skip it
    silently and restore the action's default.

    The documents are a parameter so the reading can be driven with
    synthetic workflows. Reading the repository's own is the default
    rather than the only option, which keeps the filesystem access at one
    named boundary instead of inside the derivation.

    Parameters
    ----------
    documents : dict[str, dict[str, typ.Any]] or None
        Parsed workflow documents keyed by file name. When None, the
        repository's own `.github/workflows` is read.

    Returns
    -------
    tuple[CoverageLane, ...]
        One entry per coverage step.
    """
    if documents is None:
        documents = workflow_documents()
    return tuple(
        lane
        for workflow, document, job_name, job in _declared_jobs(documents)
        for lane in _lanes_in_job(workflow, document, job_name, job)
    )
