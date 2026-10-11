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
import tempfile
import typing as typ
from pathlib import Path

import pytest
from cmd_mox import CmdMox
from hypothesis import given, settings
from hypothesis import strategies as st
from release_admission_test_cases import FAILURE_CASES
from release_admission_test_scenarios import (
    FailureCase,
    install_case,
    is_workflow_run_query,
)
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    GITHUB_REPOSITORY,
    METRICS_VALIDATOR,
    OPERATION_SEQUENCE,
    Boundaries,
    GateRun,
    Handler,
    RunShape,
    assert_complete_records,
    assert_failure_trace_sequence,
    assert_identifiers_excluded_from_diagnostics,
    assert_identifiers_excluded_from_records,
    expected_gate_labels,
    expected_operation_labels,
    install_boundaries,
    operation_duration,
    operation_records,
    run_gate,
)

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_DIR = REPO_ROOT / ".github" / "scripts"
sys.path.insert(0, str(SCRIPTS_DIR))

pytest_plugins = ("cmd_mox.pytest_plugin",)

if typ.TYPE_CHECKING:
    from cmd_mox.ipc import Invocation

#: The environment variables the property uses to carry generated identifiers
#: that are not the revision. Production never reads these names -- there is no
#: consumer of either anywhere under ``.github/scripts`` -- so they prove
#: transport only, and the claim they support is exactly that: a value that
#: reaches the child's environment does not reach a record or a diagnostic.
#: They are named as *scenario* variables rather than as configuration so a
#: reader is not misled into thinking the gate has inputs the production code
#: does not read.
IDENTIFIER_PATH_VARIABLE = "NETSUKE_RELEASE_ADMISSION_SCENARIO_PATH"
IDENTIFIER_URL_VARIABLE = "NETSUKE_RELEASE_ADMISSION_SCENARIO_URL"

#: Identifiers that survive a round trip through a shell and a JSON record: no
#: surrogates, because the record is written as UTF-8, and no NUL, because the
#: child's environment cannot carry one.
IDENTIFIER_TEXT = st.text(
    alphabet=st.characters(blacklist_categories=("Cs",), blacklist_characters="\x00"),
    min_size=1,
    max_size=32,
)
#: Canonical Git object IDs, so a generated revision reaches the commit and
#: workflow-run queries instead of being refused as malformed first.
GIT_OBJECT_ID = st.one_of(
    st.text(alphabet="0123456789abcdef", min_size=40, max_size=40),
    st.text(alphabet="0123456789abcdef", min_size=64, max_size=64),
)

#: The workflow-run request the gate must build around a generated revision,
#: split so the property names the variable part once.
RUN_QUERY_PREFIX = f"repos/{GITHUB_REPOSITORY}/actions/runs?head_sha="
RUN_QUERY_SUFFIX = "&per_page=1"
RUN_QUERY_FILTER = ".workflow_runs[0].id // empty"


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


def _answer_workflow_run(run_id: str) -> Handler:
    """Return a handler that names *run_id* for the workflow-run query.

    The generated identifier is returned as the API's single line of output, and
    production stores that line verbatim after stripping its newline. Returning
    it here is what makes the exclusion claim below a claim about the workflow
    run the gate read rather than about an environment variable the gate never
    looks at.

    Returns
    -------
    Handler
        An answer for the run query that carries the generated identifier.
    """

    def answer(_invocation: Invocation) -> tuple[str, str, int]:
        """Succeed with the generated run identifier."""
        return f"{run_id}\n", "", 0

    return answer


def _run_under_generated_identifiers(
    revision: str, run_id: str, path: str, url: str
) -> tuple[GateRun, Boundaries]:
    """Run the gate once with the given identifiers in play.

    A controller is built per example rather than taken from the fixture: the
    doubles must be registered before replay, and Hypothesis runs every example
    inside one test function, so one fixture instance cannot serve them all.

    Parameters
    ----------
    revision
        The canonical object ID the workflow exports and the queries must carry.
    run_id
        Generated identifier the workflow-run query must answer with, so the
        value reaches the gate's own result through the production adapter.
    path, url
        Generated identifiers carried in the child's environment only. Nothing
        in production reads either name; the values exist to be transported.

    Returns
    -------
    tuple[GateRun, Boundaries]
        The completed run and the doubles whose recorded calls answer whether
        the identifiers crossed any boundary they should not have.
    """
    with (
        tempfile.TemporaryDirectory() as directory_name,
        CmdMox(verify_on_exit=False) as cmd_mox,
    ):
        boundaries = install_boundaries(cmd_mox)
        boundaries.gh.wraps(
            lambda invocation, handler: (
                _answer_workflow_run(run_id)(invocation)
                if is_workflow_run_query(invocation)
                else handler(invocation)
            )
        )
        cmd_mox.replay()
        run = run_gate(
            cmd_mox,
            pathlib.Path(directory_name),
            evidence_state="fresh",
            extra_environment={
                "GITHUB_SHA": revision,
                "NETSUKE_RELEASE_ADMISSION_ENFORCE": "false",
                IDENTIFIER_PATH_VARIABLE: f"path-{path}",
                IDENTIFIER_URL_VARIABLE: f"url-{url}",
            },
        )
        cmd_mox.verify()
    return run, boundaries


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
        Generated unbounded identifiers carried in the child's environment.

    Notes
    -----
    **The workflow-run identifier is generated and returned by the double, so
    it reaches the gate through production code rather than through an
    environment variable production ignores.** The gate's
    :func:`commands.github_find_workflow_run` captures the API's single line,
    strips its newline, and stores it on the gate result, so the value the
    exclusion set carries is the value the gate read. This is a safety
    property in the ordinary sense: the current identifier's consumption is
    limited -- ``verify_evidence`` never sees it on the states the chain
    reaches, because ``check_scan_freshness`` stops every non-fresh state and
    ``classify_evidence`` satisfies its disjunction on its first operand for
    ``fresh`` -- but a future change that copies the stored identifier into
    telemetry is exactly what this case detects.

    ``path`` and ``url`` make the weaker claim and are labelled as such: their
    variable names have no consumer anywhere under ``.github/scripts``, so they
    establish transport only. They are retained because they generate arbitrary
    text over the same domain, which the revision (restricted to hex object
    IDs) and the run identifier (one per example) do not.

    The property exercises the success path through request construction and
    lookup with canonical Git object IDs, then reports missing evidence because
    the fixture supplies no evidence provider. Newline-terminated revisions are
    covered by the failure suite: the gate strips trailing newlines from the
    resolved SHA before the equality check, so a newline revision mismatches the
    raw ``GITHUB_SHA`` it was given.
    """
    identifiers = {revision, run_id, f"path-{path}", f"url-{url}"}
    run, boundaries = _run_under_generated_identifiers(revision, run_id, path, url)

    assert run.result.returncode == 0, run.result.stderr
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert run.outputs["gate-error-category"] == "missing_evidence", (
        "the generated revision must reach the evidence check"
    )
    run_query = RUN_QUERY_PREFIX + revision + RUN_QUERY_SUFFIX
    expected_requests = [
        ["api", f"repos/{GITHUB_REPOSITORY}/commits/{revision}", "--jq", ".sha"],
        ["api", run_query, "--jq", RUN_QUERY_FILTER],
    ]
    assert boundaries.gh.calls == expected_requests, (
        "the exact admission requests must carry the generated revision"
    )
    assert boundaries.git.calls == [
        ["fetch", "--depth", "1", "--no-tags", "origin", "--", revision]
    ], "the bounded fetch must carry the generated revision"
    crossed = {
        (values.get(IDENTIFIER_PATH_VARIABLE), values.get(IDENTIFIER_URL_VARIABLE))
        for values in boundaries.git.envs
    }
    assert crossed == {(f"path-{path}", f"url-{url}")}, (
        "generated identifiers must cross the subprocess boundary in the environment"
    )
    # The records must first be shown to exist. Both shared validators and the
    # exclusion helper are satisfied by an empty list, so without this the
    # exclusion claim below would hold vacuously on a gate that wrote nothing.
    assert_complete_records(
        run.metrics,
        run.traces,
        RunShape(OPERATION_SEQUENCE, "failure", "missing_evidence"),
    )
    assert_identifiers_excluded_from_records(
        run.metrics, run.traces, identifiers, subject="generated identifiers"
    )
    assert_identifiers_excluded_from_diagnostics(
        run.result.stderr, identifiers, subject="generated identifiers"
    )
