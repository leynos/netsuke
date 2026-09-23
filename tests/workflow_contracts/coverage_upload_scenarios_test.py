"""The CodeScene upload runs for exactly the events that may publish.

`coverage_upload_guard_test` holds the upload step's condition to its shape.
This module runs that condition, as written in `coverage-main.yml`, against
the events the workflow answers: a push to `main`, a dispatch from `main`, a
dispatch from a feature branch, and each of those without the credential. The
evaluator in `actions_expressions` models exactly the guard's grammar and
refuses anything else, so a condition that drifted outside it fails here
rather than being read as "never runs".

No Actions runner is involved. What decides whether the upload runs is this
condition over those contexts, so evaluating it is the upload boundary. What
the upload action does once it runs is upstream's to test.

Run via ``make test-workflow-contracts``.
"""

import pytest
from actions_expressions import UnsupportedExpressionError, evaluate_conjunction
from workflow_loading import (
    COVERAGE_MAIN_WORKFLOW_PATH,
    job_steps,
    load_workflow,
    named_step,
)

UPLOAD_STEP = "Upload coverage data to CodeScene"
MAIN = "refs/heads/main"
FEATURE = "refs/heads/feature/coverage-experiment"


def _upload_condition() -> str:
    """Return the upload step's condition as the workflow declares it."""
    steps = job_steps(load_workflow(COVERAGE_MAIN_WORKFLOW_PATH), "coverage-upload")
    condition = named_step(steps, UPLOAD_STEP).get("if")
    assert isinstance(condition, str), f"{UPLOAD_STEP} must declare a condition"
    return condition


@pytest.mark.parametrize(
    ("event", "ref", "token", "outcome"),
    [
        pytest.param("push", MAIN, "secret", "uploads", id="main-push"),
        pytest.param(
            "workflow_dispatch", MAIN, "secret", "uploads", id="main-dispatch"
        ),
        pytest.param(
            "workflow_dispatch", FEATURE, "secret", "skips", id="feature-dispatch"
        ),
        pytest.param("push", MAIN, "", "skips", id="main-push-without-token"),
        pytest.param(
            "workflow_dispatch", FEATURE, "", "skips", id="feature-without-token"
        ),
    ],
)
def test_the_upload_runs_only_where_it_may_publish(
    event: str, ref: str, token: str, outcome: str
) -> None:
    """Upload for main with the credential, and in no other case."""
    # The check step writes `available=${{ secrets.CS_ACCESS_TOKEN != '' }}`,
    # which renders as the string `true` or `false`.
    available = "true" if token else "false"
    contexts = {
        "steps": {"codescene_token.outputs.available": available},
        "github": {"event_name": event, "ref": ref},
    }
    uploads = evaluate_conjunction(_upload_condition(), contexts)
    assert ("uploads" if uploads else "skips") == outcome, (
        f"a {event} on {ref} {'with' if token else 'without'} the credential "
        f"must {outcome.removesuffix('s')}"
    )


@pytest.mark.parametrize(
    "condition",
    [
        "env.CS_ACCESS_TOKEN != '' || github.ref == 'refs/heads/main'",
        "github.ref == 'refs/heads/main' && startsWith(github.ref, 'refs/')",
        "github.ref == 'refs/heads/main' && false",
        "env.codescene_token.outputs.available == 'true'",
        "steps.codescene_token == 'true'",
    ],
)
def test_the_evaluator_refuses_what_it_does_not_model(condition: str) -> None:
    """Refuse a disjunction or an unmodelled clause rather than answer False."""
    contexts = {"env": {}, "github": {"ref": MAIN}}
    with pytest.raises(UnsupportedExpressionError):
        evaluate_conjunction(condition, contexts)
