r"""Verify release-admission metrics stay bounded.

The fixed failure-category matrix pins operation and category pairs to bounded
labels. The Hypothesis property uses canonical Git object IDs to reach commit
and workflow-run requests, has the doubled API answer the run query with a
generated identifier so that value reaches the gate through production, keeps
generated identifiers out of metric labels, trace fields, and diagnostics, and
asserts the run's record shape first so that the exclusion cannot hold
vacuously on an empty artefact. The bound the gate places on its own adapters is
exercised separately, against a real child that refuses to leave, in
:mod:`test_release_admission_process_bounds`.

The property's mechanism -- its strategies, the run it drives, and the claims
it makes about one run -- lives in
:mod:`release_admission_test_boundedness`, split out when this module reached
the project's file-size ceiling. What remains here is the two tests and the
argument for each.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with hypothesis==6.151.9 \
        --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' --with 'cyclopts==4.25.3' \
        python -m pytest \
        scripts/tests/test_release_admission_metric_boundedness.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import pathlib
import sys
import typing as typ
from pathlib import Path

import pytest
from hypothesis import given, settings
from release_admission_test_boundedness import (
    GIT_OBJECT_ID,
    IDENTIFIER_TEXT,
    assert_gate_run_is_valid,
    assert_identifiers_crossed_only_the_environment,
    assert_identifiers_never_reached_the_records,
    assert_revision_reached_the_boundaries,
    run_under_generated_identifiers,
    stored_identifier,
)
from release_admission_test_cases import FAILURE_CASES
from release_admission_test_scenarios import FailureCase, install_case
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    METRICS_VALIDATOR,
    assert_failure_trace_sequence,
    expected_gate_labels,
    expected_operation_labels,
    operation_duration,
    operation_records,
    run_gate,
)

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_DIR = REPO_ROOT / ".github" / "scripts"
sys.path.insert(0, str(SCRIPTS_DIR))

pytest_plugins = ("cmd_mox.pytest_plugin",)

if typ.TYPE_CHECKING:
    from cmd_mox import CmdMox


@pytest.mark.parametrize("case", FAILURE_CASES)
def test_gate_emits_fixed_categories_for_failure_paths(
    cmd_mox: CmdMox,
    tmp_path: pathlib.Path,
    case: FailureCase,
) -> None:
    """Verify every controlled failure retains a bounded metric category.

    Parameters
    ----------
    cmd_mox
        Active controller whose shims stand in for every adapter.
    tmp_path
        Isolated output directory for the gate.
    case
        One failure input and its documented fixed category.

    Notes
    -----
    Every failure must emit operation, gate, and workflow-output results before
    the admission script exits unsuccessfully.
    """
    install_case(cmd_mox, case)
    run = run_gate(
        cmd_mox,
        tmp_path,
        evidence_state=case.evidence_state,
        extra_environment=case.environment_for(),
    )

    assert run.result.returncode == (1 if case.enforce else 0), (
        "enforcement must fail closed while observation must retain diagnostics"
    )
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    record = operation_records(run.metrics, case.operation)[-1]
    assert record["labels"] == expected_operation_labels(
        CANARY_BY_OPERATION[case.operation],
        case.operation,
        "failure",
        case.error_category,
    ), f"{case.operation} must retain its fixed error category"
    assert run.metrics[-1]["labels"] == expected_gate_labels(
        "failure", case.error_category
    ), "the gate must retain the operation's error category"
    assert run.outputs["gate-outcome"] == "failure", (
        "failed operations must reach the workflow summary output"
    )
    assert run.outputs["gate-error-category"] == case.error_category, (
        "failed operations must retain their bounded category in workflow output"
    )
    assert_failure_trace_sequence(run.traces, case.operation, case.error_category)
    if case.run_lookup_succeeds:
        lookup = operation_records(run.metrics, "fetch_workflow_run")[-1]
        assert lookup["labels"] == expected_operation_labels(
            CANARY_BY_OPERATION["fetch_workflow_run"],
            "fetch_workflow_run",
            "success",
            "none",
        ), "an empty run identifier must reach evidence verification"
    if case.error_category == "timeout":
        assert operation_duration(run.metrics, case.operation) > 0, (
            "timed-out operations must retain a positive measured duration"
        )


@given(
    revision=GIT_OBJECT_ID,
    run_id=IDENTIFIER_TEXT,
    path=IDENTIFIER_TEXT,
    url=IDENTIFIER_TEXT,
)
@settings(deadline=None, max_examples=20)
def test_identifiers_never_become_metric_labels(
    revision: str,
    run_id: str,
    path: str,
    url: str,
) -> None:
    """Verify generated identifiers stay out of the gate's records.

    Parameters
    ----------
    revision
        A canonical 40-character SHA-1 or 64-character SHA-256 object ID.
    run_id
        A generated identifier the workflow-run query answers with.
    path, url
        Generated identifiers carried in the child's environment.

    Notes
    -----
    **The workflow-run identifier is generated and returned by the double, so
    it reaches the gate through production code rather than through an
    environment variable production ignores.** The gate's
    :func:`commands.github_find_workflow_run` captures the API's single line,
    strips every trailing newline, and stores it on the gate result, so the
    value the exclusion set carries is the value the gate stored. The generated
    identifier ends in the marker precisely so that strip cannot change it: a
    value ending in a newline would be stored as a different string from the one
    searched for, and a leak of the stored value would slip through the
    exclusion unmatched.

    This is a safety property in the ordinary sense. The stored identifier
    *does* reach a consumer -- ``gate._dispatch`` passes it as the
    ``workflow_run_id`` argument of :func:`policy.classify_evidence` -- but
    ``classify_evidence`` only ever *evaluates* it on a path this run does not
    reach: for ``fresh`` its disjunction satisfies on the first operand, and
    every non-fresh state stops earlier still, at ``check_scan_freshness``.
    Argument delivery and truthiness evaluation are different boundaries, and
    only the second is unreached here. A future change that copies the stored
    identifier into telemetry is exactly what this case detects.

    ``path`` and ``url`` make the weaker claim and are labelled as such: their
    variable names have no consumer anywhere under ``.github/scripts``, so they
    establish transport only. They are retained because they generate arbitrary
    text over the same domain, which the revision (restricted to hex object
    IDs) and the run identifier (one per example) do not.

    The exclusion set carries the run identifier **both as generated and as
    production stores it**. The generated value's trailing marker makes the two
    identical, but asserting both makes the exclusion cover the stored value
    even if the strategy changes: a value ending in a newline is stored
    normalized, so searching for the raw value alone would let a leak of the
    stored one pass unmatched. That is the dangerous direction for an exclusion
    assertion, and it is silent.

    The property exercises the success path through request construction and
    lookup with canonical Git object IDs, then reports missing evidence because
    the fixture supplies no evidence provider. Newline-terminated revisions are
    covered by the failure suite: the gate strips trailing newlines from the
    resolved SHA before the equality check, so a newline revision mismatches the
    raw ``GITHUB_SHA`` it was given.
    """
    identifiers: set[str] = {revision, run_id, stored_identifier(run_id)}
    identifiers |= {f"path-{path}", f"url-{url}"}
    run, boundaries = run_under_generated_identifiers(revision, run_id, path, url)

    assert_gate_run_is_valid(run)
    assert_revision_reached_the_boundaries(boundaries, revision)
    assert_identifiers_crossed_only_the_environment(boundaries, path, url)
    assert_identifiers_never_reached_the_records(run, identifiers)
