"""Contract tests for the trunk-only guard on the CodeScene upload.

`coverage-main.yml` runs on a push to `main` and on `workflow_dispatch`, which
the warm-run diagnostics need. The push trigger's branch filter constrains only
the first. A dispatch runs from whichever branch started it, and the upload
action does not check the ref, so without a ref clause on the step a dispatch
from a feature branch uploads that branch's report to CodeScene. The guard is
therefore asserted on the step, and read the way GitHub reads it: `&&` binds
tighter than `||`, so one disjunct anywhere authorizes the upload alone.

The file also holds the loader's refusal of a repeated key, since every
contract here, this one included, reads the workflow through it.

Run via ``make test-workflow-contracts``.
"""

import pytest
from ci_coverage_wiring_invariants import is_trunk_only_upload
from workflow_loading import (
    COVERAGE_MAIN_WORKFLOW_PATH,
    WorkflowReadError,
    job_steps,
    load_workflow,
    named_step,
    parse_workflow_text,
)

CODESCENE_UPLOAD_STEP = "Upload coverage data to CodeScene"
CREDENTIAL_PRESENT = "steps.codescene_token.outputs.available == 'true'"
MAIN_CLAUSE = "github.ref == 'refs/heads/main'"
DISPATCH = "github.event_name == 'workflow_dispatch'"
SAME_REPOSITORY = "github.repository == 'leynos/netsuke'"


def test_the_upload_runs_only_from_the_trunk_ref() -> None:
    """Require the CodeScene upload to carry both the token and the ref clause."""
    steps = job_steps(load_workflow(COVERAGE_MAIN_WORKFLOW_PATH), "coverage-upload")
    condition = named_step(steps, CODESCENE_UPLOAD_STEP).get("if")
    assert is_trunk_only_upload(condition), (
        f"the CodeScene upload must require {CREDENTIAL_PRESENT} and "
        f"{MAIN_CLAUSE} with no disjunction; a dispatch from any branch "
        f"uploads otherwise, got {condition!r}"
    )


@pytest.mark.parametrize(
    "condition",
    [
        f"{CREDENTIAL_PRESENT} && {MAIN_CLAUSE}",
        f"{MAIN_CLAUSE} && {CREDENTIAL_PRESENT}",
        f"  {CREDENTIAL_PRESENT}   &&\n  {MAIN_CLAUSE} ",
        f"{CREDENTIAL_PRESENT} && {MAIN_CLAUSE} && {SAME_REPOSITORY}",
    ],
)
def test_a_conjunction_of_both_clauses_is_accepted(condition: str) -> None:
    """Accept both clauses in any order or spacing, and any further narrowing."""
    assert is_trunk_only_upload(condition), f"{condition!r} must be accepted"


@pytest.mark.parametrize(
    "condition",
    [
        # Each disjunct survives the split: both clauses are still whole
        # conjuncts, so only the refusal of `||` rejects these. The first lets
        # a dispatch upload from any branch; the second drops the token clause.
        f"{DISPATCH} || {MAIN_CLAUSE} && {CREDENTIAL_PRESENT} && {MAIN_CLAUSE}",
        f"{MAIN_CLAUSE} || {DISPATCH} && {CREDENTIAL_PRESENT} && {MAIN_CLAUSE}",
        f"({DISPATCH} || true) && {CREDENTIAL_PRESENT} && {MAIN_CLAUSE}",
        # The required clauses stay whole and first; the `||` hides in what
        # reads as one more narrowing conjunct, which the split permits.
        (f"{CREDENTIAL_PRESENT} && {MAIN_CLAUSE} && github.actor != 'x' || {DISPATCH}"),
    ],
)
def test_a_disjunction_is_refused_even_beside_both_clauses(condition: str) -> None:
    """Refuse any unquoted `||`, even where both clauses are present."""
    assert not is_trunk_only_upload(condition), f"{condition!r} must be refused"


@pytest.mark.parametrize(
    "condition",
    [
        CREDENTIAL_PRESENT,
        MAIN_CLAUSE,
        f"!({CREDENTIAL_PRESENT}) && {MAIN_CLAUSE}",
        f"{CREDENTIAL_PRESENT} && contains({MAIN_CLAUSE!r}, 'main')",
        # Both clauses sit inside a quoted literal (the doubled quote is an
        # escaped quote) within a negated group; the trailing `&& false` makes
        # the group true for every token and ref. A naive split on `&&` would
        # read both clauses as conjuncts of the whole.
        (
            "true && !('note && env.CS_ACCESS_TOKEN != '' && ' && "
            "github.ref == 'refs/heads/main' && false)"
        ),
        f"true && !({CREDENTIAL_PRESENT} && {MAIN_CLAUSE} && false)",
        None,
    ],
)
def test_a_missing_or_disguised_clause_is_refused(condition: object) -> None:
    """Refuse a clause that is absent, negated, quoted, or nested in a group.

    Every form but the bare ones contains each clause's text, so a substring
    test would accept them, and the grouped ones defeat a naive split on `&&`
    too. Only top-level conjuncts, compared whole, count.
    """
    assert not is_trunk_only_upload(condition), f"{condition!r} must be refused"


def test_the_loader_refuses_a_repeated_key() -> None:
    """Fail on `runs-on` declared twice rather than keeping the second label.

    PyYAML's default is to keep the last value silently, so a paid label in
    the discarded half would read as hosted to every placement contract.
    """
    doubled = (
        "on: pull_request\n"
        "jobs:\n"
        "  build:\n"
        "    runs-on: ubicloud-standard-4-ubuntu-2404\n"
        "    runs-on: ubuntu-latest\n"
    )
    with pytest.raises(WorkflowReadError, match="runs-on"):
        parse_workflow_text(doubled, "doubled workflow")


def test_the_loader_keeps_distinct_keys() -> None:
    """Keep `on` and a boolean-looking key apart, so only real repeats fail."""
    parsed = parse_workflow_text("on: push\n'true': 1\njobs: {}\n", "workflow")
    assert parsed == {"on": "push", "true": 1, "jobs": {}}, (
        f"distinct keys must survive the strict loader, got {parsed!r}"
    )
