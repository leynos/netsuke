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
from release_publish_path_context import job_contexts
from release_publish_path_scenario_model import GITHUB_SHA, Scenario
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
#: The rehearsal flag a generated run reports. It is fixed rather than drawn:
#: the policy never emits a publication with a dry run beside it, so drawing
#: both would generate an unreachable pair whose guards legitimately disagree
#: -- only the write job carries the veto. ``release_publish_path_authorization``
#: exercises that veto directly, and its mutation proves the check notices when
#: the clause is dropped.
NO_DRY_RUN = "false"


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
def _generated_run(draw: st.DrawFn) -> tuple[Scenario, dict[str, str]]:
    """Draw one run: each dependency's result, the gate's outputs, and the mode.

    The admission outputs are drawn separately from the enforcement mode the
    real workflow derives, because the contract under test is the guard's own
    comparison rather than the step that feeds it.

    Returns
    -------
    tuple[Scenario, dict[str, str]]
        The scenario, whose declared results are the drawn dependency results,
        and the metadata outputs the guards read.
    """
    results = {name: draw(st.sampled_from(RESULTS)) for name in _dependency_names()}
    admission = {
        "enforcement": draw(st.sampled_from(ENFORCEMENTS)),
        "gate_outcome": draw(st.sampled_from(GATE_OUTCOMES)),
        "candidate_commit": draw(st.sampled_from(CANDIDATES)),
    }
    outputs = {
        "should_publish": draw(st.sampled_from(SHOULD_PUBLISH)),
        "dry_run": NO_DRY_RUN,
    }
    scenario = Scenario(
        name="generated",
        mode="publish",
        event_action="",
        needs_results=results,
        cancelled=draw(st.booleans()),
        release_runs=True,
        admission_outputs=admission,
    )
    return scenario, outputs


def _admits(
    scenario: Scenario, outputs: cabc.Mapping[str, str], needed: cabc.Iterable[str]
) -> bool:
    """Compute the rule a write job's guard must express for this run."""
    gate = scenario.gate_outputs
    return (
        not scenario.cancelled
        and all(scenario.effective_results.get(name) == "success" for name in needed)
        and gate.get("enforcement") == "true"
        and gate.get("gate_outcome") == "success"
        and gate.get("candidate_commit") == GITHUB_SHA
        and outputs.get("should_publish") == "true"
    )


@given(run=_generated_run())
def test_a_write_job_starts_exactly_when_the_whole_gate_holds(
    run: tuple[Scenario, dict[str, str]],
) -> None:
    """Require every write guard to agree with the admission rule."""
    scenario, outputs = run
    violations: list[str] = []
    jobs = write_authority_jobs(_release())
    assert jobs, "the workflow must declare at least one write-authority job"
    for name, job in sorted(jobs.items()):
        needed = job_names(job.get("needs", [])) or []
        value = evaluate_field(
            job.get("if", True),
            FieldEvaluation(
                contexts=job_contexts(scenario, outputs, job),
                violations=violations,
                label=f"generated.{name}.guard",
                job_level=True,
            ),
        )
        runs = value is not MISSING and bool(value)
        expected = _admits(scenario, outputs, needed)
        assert runs == expected, (
            f"{name} resolved to {runs!r}, expected {expected!r} for "
            f"results={scenario.effective_results!r}, "
            f"guard violations={violations!r}"
        )
    assert not violations, f"every guard must evaluate cleanly, saw {violations!r}"
