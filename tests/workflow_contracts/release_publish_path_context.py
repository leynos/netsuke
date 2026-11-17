"""Build the per-scenario state one release guard is evaluated against.

A guard reads the scenario's mode and trigger, the metadata and admission
outputs it compares, and the results of the jobs it declares. Carrying those
together keeps each check's call surface small, and makes it impossible to
evaluate one scenario's guard against another scenario's results.

The ``needs`` context is built from the job's own declaration rather than a
required set, so a mutated dependency list is evaluated as written, and the
results come from :func:`~release_publish_path_results.resolve_results`, so a
dependency no scenario names is reported as skipped rather than assumed
successful.

The module lives beside the scenario table rather than inside it because the
table and the model have separate reasons to change, and keeping them apart
holds each file inside the project's size ceiling.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import typing as typ

from release_publish_path_scenarios import Scenario, admission_outputs

if typ.TYPE_CHECKING:
    import collections.abc as cabc


@dataclasses.dataclass(frozen=True, slots=True)
class ScenarioRun:
    """Bundle one scenario with the outputs and results its guards read."""

    scenario: Scenario
    outputs: dict[str, str]
    results: dict[str, str]


def needs_context(names: cabc.Iterable[str], run: ScenarioRun) -> dict[str, object]:
    """Build one ``needs`` entry per job, carrying the outputs its guard reads.

    Returns
    -------
    dict[str, object]
        A ``needs`` mapping with each dependency's resolved result, plus the
        metadata and admission outputs their guards compare.
    """
    needs: dict[str, object] = {}
    for name in names:
        dependency: dict[str, object] = {"result": run.results.get(name, "skipped")}
        match name:
            case "metadata":
                dependency["outputs"] = dict(run.outputs)
            case "release-admission-canaries":
                dependency["outputs"] = admission_outputs(run.scenario)
        needs[name] = dependency
    return needs


__all__ = ("ScenarioRun", "needs_context")
