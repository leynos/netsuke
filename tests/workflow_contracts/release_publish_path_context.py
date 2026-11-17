"""Build the ``needs`` context one scenario's guard is evaluated against.

A guard reads the results of the jobs it declares and, for the metadata and
admission jobs, the outputs it compares. The context is built from the job's
own declaration rather than a required set, so a mutated dependency list is
evaluated as written, and the results come from
:func:`~release_publish_path_results.resolve_results`, so a dependency no
scenario names is reported as skipped rather than assumed successful.

The module lives beside the scenario table rather than inside it because the
table and the model have separate reasons to change, and keeping them apart
holds each file inside the project's size ceiling.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_publish_path_scenarios import Scenario, admission_outputs

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def needs_context(
    names: cabc.Iterable[str],
    scenario: Scenario,
    metadata_outputs: cabc.Mapping[str, str],
    results: cabc.Mapping[str, str],
) -> dict[str, object]:
    """Build one ``needs`` entry per job, carrying the outputs its guard reads.

    Returns
    -------
    dict[str, object]
        A ``needs`` mapping with each dependency's resolved result, plus the
        metadata and admission outputs their guards compare.
    """
    needs: dict[str, object] = {}
    for name in names:
        dependency: dict[str, object] = {"result": results.get(name, "skipped")}
        match name:
            case "metadata":
                dependency["outputs"] = dict(metadata_outputs)
            case "release-admission-canaries":
                dependency["outputs"] = admission_outputs(scenario)
        needs[name] = dependency
    return needs


__all__ = ("needs_context",)
