"""Reads every coverage-invoking lane out of the workflow files.

Separated from ``timeout_budgets`` so the workflow reading and the
nextest arithmetic stay legible apart, and from ``lane_environment``,
which resolves a variable across the step, job and workflow scopes, so
no module outgrows the 400-line limit the Python lint gate enforces.
"""

import fractions
import typing as typ

from lane_environment import WatchdogValueError, nextest_profile_of, watchdog_of
from timeout_budgets import COVERAGE_ACTION, WORKFLOWS_DIRECTORY
from workflow_loading import all_workflow_documents

if typ.TYPE_CHECKING:
    import collections.abc as cabc


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
    watchdog : fractions.Fraction or None
        The watchdog budget in seconds exactly, or None when the job
        sets none
        and so inherits the action's 1,800 s default.
    job_timeout : fractions.Fraction or None
        The job's ``timeout-minutes`` in seconds, or None when it
        declares none and so inherits GitHub's six-hour default.
    condition : tuple[object, object]
        The ``if`` on the coverage step and on its job. A skipped step
        runs no ``cargo``, so its watchdog never arms and every budget
        below says nothing about it.
    nextest_profile : str or None
        The nextest profile the lane selects, or None when it names
        none and so runs under ``default``. The whole-run budget lives
        in one profile, so a lane naming another is a lane that budget
        does not reach.
    """

    workflow: str
    job: str
    step: str
    watchdog: fractions.Fraction | None
    job_timeout: fractions.Fraction | None
    condition: tuple[object, object] = (None, None)
    nextest_profile: str | None = None

    def __str__(self) -> str:
        """Return a location suitable for a failure message.

        Returns
        -------
        str
            ``workflow:job:step`` for this lane.
        """
        return f"{self.workflow}:{self.job}:{self.step!r}"


def workflow_documents() -> dict[str, dict[str, typ.Any]]:
    """Return every workflow document in the repository, keyed by name.

    Delegates to ``workflow_loading.all_workflow_documents``, the
    boundary the rest of this suite already reads workflows through.
    This module parsed them a second time with ``yaml.safe_load``, which
    is a YAML 1.1 loader and so reads the ``on:`` trigger key as
    ``True``, and which reported an unreadable file with whatever
    exception the failure happened to raise.

    Every derivation below takes its documents as a parameter, so this
    call is the only filesystem access the lane reading performs.

    A workflow file that cannot be read or parsed propagates
    :class:`workflow_loading.WorkflowReadError` from the boundary. A
    budget derived from the workflows this contract could see, while one
    it could not was silently dropped, would assert nothing about that
    file.

    Returns
    -------
    dict[str, dict[str, typ.Any]]
        File name to parsed document.
    """
    return typ.cast(
        "dict[str, dict[str, typ.Any]]", all_workflow_documents(WORKFLOWS_DIRECTORY)
    )


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


def _watchdog_windows(step: dict[str, typ.Any]) -> int:
    """Return how many cargo watchdog windows one coverage step arms."""
    # The rationale lives where it is asserted, so that it is stated once:
    # `test_a_doctests_step_arms_the_watchdog_twice` for the two windows,
    # its declining counterpart for the spellings read as one, and "Test
    # timeouts: the tiers this repository sets" in `docs/developers-guide.md`
    # for the measurement behind both.
    #
    # A class pattern rather than ``case {"doctests": "true"}``: mapping
    # patterns test ``PyMapping_Check``, which a non-`dict` mapping passes,
    # so the literal form reads such a step as two windows where the
    # `isinstance` guard it replaced read one. ``dict()`` accepts exactly
    # the subject that guard did.
    match step.get("with"):
        case dict() as inputs if inputs.get("doctests") == "true":
            return 2
        case _:
            return 1


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
        One entry per watchdog window a coverage step arms: two for a
        step asking the action for the doctest pass, one otherwise. The
        two entries share every field, because they are one step; what
        differs is the second `cargo` invocation the job's ceiling has
        to contain.

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
    # Read from the text and multiplied exactly. This ceiling is
    # compared against a sum of watchdog budgets and two allowances, so
    # a float here would discard the exactness the other terms carry;
    # the minute-to-second conversion is itself a term of that
    # comparison rather than a display detail.
    timeout = None if raw_timeout is None else fractions.Fraction(str(raw_timeout)) * 60
    return [
        CoverageLane(
            workflow=workflow,
            job=job_name,
            step=str(step.get("name", "")) or job_name,
            watchdog=watchdogs[index],
            job_timeout=timeout,
            condition=(step.get("if"), job.get("if")),
            nextest_profile=nextest_profile_of(document, job, step),
        )
        for index, step in enumerate(steps)
        # Repeated rather than built twice: the windows of one step are
        # that step's single coordinate, one per `cargo` invocation.
        for _ in range(_watchdog_windows(step))
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
        One entry per watchdog window a coverage step arms: two for a
        step asking the action for the doctest pass, one otherwise. The
        two entries are one step and share every field.
    """
    if documents is None:
        documents = workflow_documents()
    return tuple(
        lane
        for workflow, document, job_name, job in _declared_jobs(documents)
        for lane in _lanes_in_job(workflow, document, job_name, job)
    )


def conditions_by_coordinate(
    lanes: cabc.Iterable[CoverageLane],
) -> dict[tuple[str, str, str], tuple[tuple[object, object], ...]]:
    """Return each coordinate's conditions, in document order.

    Parameters
    ----------
    lanes : cabc.Iterable[CoverageLane]
        The lanes to group.

    Returns
    -------
    dict
        Workflow, job and step name to the conditions declared there.
    """
    # A coordinate can hold more than one lane. An unnamed coverage step
    # takes its job's name, so two of them in one job share a
    # coordinate, and a mapping from coordinate to a single condition
    # keeps only the last: a step skipped by `if: false` beside one
    # carrying the expected condition then passes unexamined. Keeping
    # the conditions as a sequence makes that collision fail instead.
    grouped: dict[tuple[str, str, str], list[tuple[object, object]]] = {}
    for lane in lanes:
        grouped.setdefault((lane.workflow, lane.job, lane.step), []).append(
            lane.condition
        )
    return {coordinate: tuple(found) for coordinate, found in grouped.items()}
