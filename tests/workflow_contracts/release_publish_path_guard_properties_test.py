"""Hold every write job's guard to the admission rule over generated runs.

The scenario table names the runs the release graph is expected to meet. This
module checks the complementary claim: for *any* combination of prerequisite
results, admission outputs, and run mode, a write job starts exactly when
every dependency it declares succeeded, the gate admitted the run's own
candidate, the run is publishing, and the run was not cancelled.

The expected answer is computed in Python from the generated values, while the
actual answer comes from evaluating the workflow's own ``if`` expression, so a
guard that drifts from the rule is a generated counterexample rather than a
hand-picked case someone forgot to add. The write jobs themselves are derived
from the workflow's declared permissions, so a publisher added beside the gate
is covered without editing this module.

Run via ``make test-workflow-contracts``.
"""

import functools
import typing as typ

from hypothesis import given
from hypothesis import strategies as st
from release_publish_needs import job_names
from release_publish_path_artifacts import MISSING, FieldEvaluation, evaluate_field
from release_publish_path_context import ScenarioRun, needs_context
from release_publish_path_scenarios import GITHUB_SHA, Scenario, admission_outputs
from release_write_authority_contract import write_authority_jobs
from workflow_loading import REPO_ROOT, load_workflow

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The release workflow whose guards are evaluated.
RELEASE_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"
#: Every result a dependency can report, including a cancelled dependency.
RESULTS = ("success", "failure", "skipped", "cancelled")
#: Enforcement values, including an unset spelling the gate never admits on.
ENFORCEMENTS = ("true", "false", "")
#: Gate outcomes that admit and that refuse.
GATE_OUTCOMES = ("success", "failure")
#: The run's own candidate, and a candidate that must never be admitted.
CANDIDATES = (GITHUB_SHA, "b" * 40)
#: The metadata output every publishing guard compares.
SHOULD_PUBLISH = ("true", "false")


@functools.cache
def _release() -> dict[str, object]:
    """Return the parsed release workflow."""
    return load_workflow(RELEASE_WORKFLOW_PATH)


@functools.cache
def _dependency_names() -> tuple[str, ...]:
    """Return every job name the derived write jobs wait on."""
    names = {
        need
        for job in write_authority_jobs(_release()).values()
        for need in job_names(job.get("needs", [])) or []
    }
    return tuple(sorted(names))


@st.composite
def _generated_run(draw: st.DrawFn) -> ScenarioRun:
    """Draw one run: each dependency's result, the gate's outputs, and the mode.

    The admission outputs are drawn separately from the enforcement mode the
    real workflow derives, because the contract under test is the guard's own
    comparison rather than the step that feeds it.

    Returns
    -------
    ScenarioRun
        One run whose results, admission outputs, and mode are drawn.
    """
    results = {name: draw(st.sampled_from(RESULTS)) for name in _dependency_names()}
    admission = {
        "enforcement": draw(st.sampled_from(ENFORCEMENTS)),
        "gate_outcome": draw(st.sampled_from(GATE_OUTCOMES)),
        "candidate_commit": draw(st.sampled_from(CANDIDATES)),
    }
    outputs = {"should_publish": draw(st.sampled_from(SHOULD_PUBLISH))}
    scenario = Scenario(
        name="generated",
        mode="publish",
        event_action="",
        needs_results=results,
        cancelled=draw(st.booleans()),
        release_runs=True,
        admission_outputs=admission,
    )
    return ScenarioRun(scenario, outputs, results)


def _admits(run: ScenarioRun, needed: cabc.Iterable[str]) -> bool:
    """Compute the rule a write job's guard must express for this run."""
    outputs = admission_outputs(run.scenario)
    return (
        not run.scenario.cancelled
        and all(run.results.get(name) == "success" for name in needed)
        and outputs.get("enforcement") == "true"
        and outputs.get("gate_outcome") == "success"
        and outputs.get("candidate_commit") == GITHUB_SHA
        and run.outputs.get("should_publish") == "true"
    )


def _guard_contexts(run: ScenarioRun, needed: list[str]) -> dict[str, object]:
    """Build the guard context from the job's own declared dependencies."""
    return {
        "needs": needs_context(needed, run),
        "status": {"cancelled": run.scenario.cancelled},
        "github": {"event": {"action": run.scenario.event_action}, "sha": GITHUB_SHA},
    }


@given(run=_generated_run())
def test_a_write_job_starts_exactly_when_the_whole_gate_holds(
    run: ScenarioRun,
) -> None:
    """Require every write guard to agree with the admission rule."""
    violations: list[str] = []
    jobs = write_authority_jobs(_release())
    assert jobs, "the workflow must declare at least one write-authority job"
    for name, job in sorted(jobs.items()):
        needed = job_names(job.get("needs", [])) or []
        value = evaluate_field(
            job.get("if", True),
            FieldEvaluation(
                contexts=_guard_contexts(run, needed),
                violations=violations,
                label=f"generated.{name}.guard",
                job_level=True,
            ),
        )
        runs = value is not MISSING and bool(value)
        expected = _admits(run, needed)
        assert runs == expected, (
            f"{name} resolved to {runs!r}, expected {expected!r} for "
            f"results={run.results!r}, guard violations={violations!r}"
        )
    assert not violations, f"every guard must evaluate cleanly, saw {violations!r}"
