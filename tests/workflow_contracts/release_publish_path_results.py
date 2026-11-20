"""Derive every release job's result from one scenario's declared outcomes.

A scenario names the outcomes whose failure it exists to exercise. Naming a
downstream job's result the same way let a case assert that a publisher
succeeded while the build it publishes had failed, which GitHub cannot produce:
a job does not run once a dependency it needs has not succeeded. This module
derives the remaining results from the workflow's own edges, so every scenario
describes a run the graph could actually have.

Two rules decide whether a job runs. A job that declares no guard keeps
GitHub's implicit rule and runs only when every dependency succeeded; a job
that declares one is evaluated as written, which is why the rehearsal can still
run after packaging or admission fails -- it does not need them. A job that
runs reports the result the scenario declares for it, defaulting to success.

A cancelled run is modelled through the status rather than through these
results: the cancellation scenarios cancel after every dependency passed, so
the results stay as the graph resolved them and only ``!cancelled()`` stops a
job. Folding the cancellation into each result instead would make every guard
false for the wrong reason, and a guard that dropped ``!cancelled()`` would
then look correct. One dependency may still report ``cancelled`` while the run
continues, as a job-level concurrency rule cancelling a pending job does; that
is an ordinary non-successful result here and propagates through the rule above.
The scenario table therefore varies each prerequisite's result through
``failure``, ``skipped``, and ``cancelled``, rather than modelling cancellation
only as a cancelled run, which would make every guard false for one reason and
hide a guard that tested ``!= 'failure'`` instead of ``== 'success'``.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import typing as typ

from release_publish_needs import job_names
from release_publish_path_artifacts import MISSING, FieldEvaluation, evaluate_field
from release_publish_path_context import Resolution, job_contexts
from workflow_loading import require_mapping

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from release_publish_path_scenario_model import Scenario


def resolve_results(
    release: dict[str, object],
    scenario: Scenario,
    metadata_outputs: dict[str, str],
    violations: list[str],
) -> dict[str, str]:
    """Return every job's result, propagated along the workflow's own edges.

    Parameters
    ----------
    release
        Parsed release workflow, whose jobs and edges define the graph.
    scenario
        The release mode and the leaf outcomes the case declares.
    metadata_outputs
        The metadata outputs the guards under evaluation read.
    violations
        Collector for an unreadable graph or guard.

    Returns
    -------
    dict[str, str]
        Each job's result as the runner would report it for this scenario.
    """
    jobs = require_mapping(release.get("jobs"), "release.jobs")
    results: dict[str, str] = {}
    state = _JobResults(scenario, metadata_outputs, results, violations)
    for name in dependency_order(jobs, violations):
        job = jobs[name]
        if not isinstance(job, dict):
            violations.append(f"release.jobs.{name}: job must be a mapping")
            continue
        results[name] = state.result_for(name, job)
    return results


def dependency_order(
    jobs: cabc.Mapping[str, object], violations: list[str]
) -> list[str]:
    """Return the jobs in dependency order, reporting a cycle rather than looping.

    Returns
    -------
    list[str]
        Every job name, each after the jobs it depends on. A graph that cannot
        be ordered still yields all names, so the caller reports on the graph
        it was given rather than on a prefix of it.
    """
    pending = {name: _declared_needs(job) & set(jobs) for name, job in jobs.items()}
    order: list[str] = []
    while pending:
        ready = sorted(name for name, needs in pending.items() if not needs)
        if not ready:
            violations.append(
                f"release.jobs: dependency cycle among {sorted(pending)!r}"
            )
            return [*order, *sorted(pending)]
        order.extend(ready)
        for name in ready:
            pending.pop(name)
        for needs in pending.values():
            needs.difference_update(ready)
    return order


def _declared_needs(job: object) -> set[str]:
    """Return a job's dependency names, treating an unreadable list as empty."""
    return (
        set(job_names(job.get("needs", [])) or []) if isinstance(job, dict) else set()
    )


@dataclasses.dataclass(frozen=True, slots=True)
class _JobResults:
    """Derive one job's result from the results already established.

    The resolver fills ``results`` in dependency order, so every job reads its
    dependencies' final answers while the map itself grows.

    Attributes
    ----------
    scenario
        The case being resolved, whose declared outcomes name the leaves.
    metadata_outputs
        The metadata outputs the guards under evaluation read.
    results
        The results derived so far, as the runner would report them.
    violations
        Collector for an unreadable guard.
    """

    scenario: Scenario
    metadata_outputs: cabc.Mapping[str, str]
    results: dict[str, str]
    violations: list[str]

    def result_for(self, name: str, job: dict[str, object]) -> str:
        """Return the named job's result, deriving whether it runs at all.

        Returns
        -------
        str
            The result the runner would report for this job: ``skipped`` when
            the graph excludes it, else the scenario's declared outcome.
        """
        if not self._runs(name, job):
            return "skipped"
        return self.scenario.needs_results.get(name, "success")

    def _runs(self, name: str, job: dict[str, object]) -> bool:
        """Return whether the job starts, given its dependencies' results."""
        needs = job_names(job.get("needs", [])) or []
        guard = job.get("if")
        if guard is None:
            return all(self.results.get(need, "success") == "success" for need in needs)
        value = evaluate_field(
            guard,
            FieldEvaluation(
                contexts=job_contexts(
                    self.scenario,
                    self.metadata_outputs,
                    job,
                    resolution=Resolution(results=self.results, cancelled=False),
                ),
                violations=self.violations,
                label=f"{self.scenario.name}.{name}.result-guard",
                job_level=True,
            ),
        )
        return value is not MISSING and bool(value)


__all__ = ("dependency_order", "resolve_results")
