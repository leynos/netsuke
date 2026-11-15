r"""Verify release-admission metrics stay bounded.

The fixed failure-category matrix pins operation and category pairs to bounded
labels. The Hypothesis property uses canonical Git object IDs and generated
run identifiers to reach the commit and run requests, has the doubled API
answer the run query with the candidate so the run is admitted, keeps generated
identifiers out of metric labels, trace fields, and diagnostics, and asserts
the run's record shape first so that the exclusion cannot hold vacuously on an
empty artefact. The bound the gate places on its own adapters is exercised
separately, against a real child that refuses to leave, in
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

import typing as typ

import pytest
from hypothesis import given, settings
from release_admission_test_boundedness import (
    GIT_OBJECT_ID,
    IDENTIFIER_TEXT,
    assert_gate_run_is_valid,
    assert_identifiers_crossed_only_the_environment,
    assert_identifiers_never_reached_the_records,
    assert_identifiers_reached_the_boundaries,
    run_under_generated_identifiers,
)
from release_admission_test_cases import FAILURE_CASES
from release_admission_test_scenarios import FailureCase, install_case
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    METRICS_VALIDATOR,
    REVISION,
    assert_failure_trace_sequence,
    expected_gate_labels,
    expected_operation_labels,
    operation_duration,
    operation_records,
    run_gate,
)

pytest_plugins = ("cmd_mox.pytest_plugin",)

if typ.TYPE_CHECKING:
    import pathlib

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
    assert run.outputs["enforcement"] == str(case.enforce).lower(), (
        "the gate must publish the mode it actually ran in"
    )
    assert run.outputs["candidate-commit"] == REVISION, (
        "the gate must publish the candidate it verified against"
    )
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
        A canonical 40-character SHA-1 object ID, the width the evidence
        manifest's ``commit`` field states.
    run_id
        A generated identifier the child's ``GITHUB_RUN_ID`` names, so the
        value reaches the run request through production.
    path, url
        Generated identifiers carried in the child's environment.

    Notes
    -----
    **The workflow-run identifier is read from the environment by production
    and placed in the run request's argument vector**, so it reaches the point
    where a record could be built around it rather than staying in a variable
    nothing reads. The doubled API answers the *head* question with the
    candidate, so the lookup succeeds and the run continues into the evidence
    operations.

    The generated identifiers each carry the marker at both ends, so they cannot
    be a substring of any fixed token the gate emits and no normalisation can
    turn a leaked value into one the search would miss.

    ``path`` and ``url`` make the weaker claim and are labelled as such: their
    variable names have no consumer anywhere under ``.github/scripts``, so they
    establish transport only. They are retained because they generate arbitrary
    text over the same domain, which the revision (restricted to hex object
    IDs) and the run identifier (one per example) do not.

    The candidate revision deliberately is *not* excused from the record
    exclusion: the workflow publishes it as ``candidate-commit``, but no metric
    or trace record may carry it, and no diagnostic may either. That is the
    distinction this property holds.
    """
    identifiers: set[str] = {revision, run_id}
    identifiers |= {f"path-{path}", f"url-{url}"}
    run, boundaries = run_under_generated_identifiers(revision, run_id, path, url)

    assert_gate_run_is_valid(run)
    assert_identifiers_reached_the_boundaries(boundaries, revision, run_id)
    assert_identifiers_crossed_only_the_environment(boundaries, path, url)
    assert_identifiers_never_reached_the_records(run, identifiers)
