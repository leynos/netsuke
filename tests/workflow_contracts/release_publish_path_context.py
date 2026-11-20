"""Build the contexts a release job's guard is evaluated against.

GitHub reports a job's dependencies through the ``needs`` context and the run's
cancellation through ``status.cancelled``. Every oracle here evaluates the
workflow's own ``if`` expression against those contexts rather than against a
second copy of its rule, so the shape a guard sees has to match what the runner
supplies: one entry per declared need, carrying that dependency's result and,
for the two jobs whose outputs a guard reads, those outputs as well.

Resolution of the graph supplies ``cancelled=False``. A cancelled run stops jobs
through ``!cancelled()`` alone, so carrying the cancellation in every
dependency's result would make each guard false for the wrong reason, and a
guard that had dropped the clause would look correct.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import typing as typ

from release_publish_needs import job_names
from release_publish_path_scenario_model import ADMISSION_JOB, GITHUB_SHA, Scenario

if typ.TYPE_CHECKING:
    import collections.abc as cabc


@dataclasses.dataclass(frozen=True, slots=True)
class Resolution:
    """State one dependency-ordered pass over the graph has established.

    Attributes
    ----------
    results
        The results already derived for the jobs evaluated before this one.
    cancelled
        Whether the run itself was cancelled. The resolver passes ``False``
        because it derives results before cancellation applies.
    """

    results: cabc.Mapping[str, str]
    cancelled: bool


def _needs_entry(
    scenario: Scenario,
    name: str,
    outputs: cabc.Mapping[str, str],
    results: cabc.Mapping[str, str],
) -> dict[str, object]:
    """Build one dependency entry as a guard expression sees it.

    Only the two jobs whose outputs a guard reads carry them: metadata supplies
    the release-mode map, and the admission gate supplies the enforcement,
    outcome and candidate every write guard compares against.

    Returns
    -------
    dict[str, object]
        The dependency's result, plus its outputs where a guard reads them. A
        job the results do not name reports ``skipped``, which is what GitHub
        reports for a dependency whose guard excluded it.
    """
    entry: dict[str, object] = {"result": results.get(name, "skipped")}
    if name == "metadata":
        entry["outputs"] = dict(outputs)
    if name == ADMISSION_JOB:
        entry["outputs"] = scenario.gate_outputs
    return entry


def job_contexts(
    scenario: Scenario,
    outputs: cabc.Mapping[str, str],
    job: dict[str, object],
    *,
    resolution: Resolution | None = None,
) -> dict[str, object]:
    """Build the contexts one job's guard is evaluated against.

    The needs are read from the job rather than from the module's constants, so
    a mutated dependency list is evaluated as written and reports its own
    result instead of being silently replaced by the oracle's expectation.

    ``resolution`` overrides the results and the cancellation flag. The
    resolver passes the map it is building, because a job's result is derived
    from the results already established for its dependencies; every other
    caller leaves it unset and reads the scenario's own state.

    Returns
    -------
    dict[str, object]
        The ``needs``, ``status`` and ``github`` contexts for the scenario.
    """
    results = scenario.effective_results if resolution is None else resolution.results
    cancelled = scenario.cancelled if resolution is None else resolution.cancelled
    return {
        "needs": {
            name: _needs_entry(scenario, name, outputs, results)
            for name in job_names(job.get("needs", [])) or []
        },
        "status": {"cancelled": cancelled},
        "github": {"event": {"action": scenario.event_action}, "sha": GITHUB_SHA},
    }


__all__ = ("Resolution", "job_contexts")
