r"""Verify release-admission metrics stay bounded.

The fixed failure-category matrix pins operation and category pairs to bounded
labels. The Hypothesis property uses canonical Git object IDs to reach commit
and workflow-run requests, then keeps generated identifiers out of metric
labels and trace fields. A final test exercises the bound the gate places on
its own adapters against a child that refuses to leave.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with hypothesis==6.151.9 \
        --with 'cmd-mox==0.2.0' --with 'cuprum==0.1.0' --with 'cyclopts==4.25.3' \
        python -m pytest \
        scripts/tests/test_release_admission_metric_boundedness.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import pathlib
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the bound under test spawns a child.
import sys
import tempfile
from pathlib import Path

import pytest
from cmd_mox import CmdMox
from hypothesis import given, settings
from hypothesis import strategies as st
from release_admission_test_cases import FAILURE_CASES
from release_admission_test_scenarios import FailureCase, install_case
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    GITHUB_REPOSITORY,
    METRICS_VALIDATOR,
    assert_failure_trace_sequence,
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
import cuprum  # ruff: ignore[module-import-not-at-top-of-file] - needs sys.path insertion.
import cuprum.sh  # ruff: ignore[module-import-not-at-top-of-file] - needs sys.path insertion.
from _release_admission import (  # ruff: ignore[module-import-not-at-top-of-file] - needs sys.path insertion.
    commands,
)

pytest_plugins = ("cmd_mox.pytest_plugin",)

#: The environment variables the property uses to carry generated identifiers
#: that are not the revision. The gate ignores them; they exist to prove that a
#: value reaching the child's environment does not reach the records.
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
    """Verify canonical revisions and arbitrary identifiers stay out of labels.

    Parameters
    ----------
    revision
        A canonical 40-character SHA-1 or 64-character SHA-256 object ID.
    run_id, path, url
        Generated unbounded identifiers that must not become labels.

    Notes
    -----
    This property exercises the success path through request construction and
    lookup with canonical Git object IDs, then reports missing evidence because
    the fixture supplies no evidence provider. Newline-terminated revisions are
    covered by the failure suite: the gate strips trailing newlines from the
    resolved SHA before the equality check, so a newline revision mismatches the
    raw ``GITHUB_SHA`` it was given.

    A controller is built per example rather than taken from the fixture: the
    doubles must be registered before replay, and Hypothesis runs every example
    inside one test function, so one fixture instance cannot serve them all.
    """
    del run_id
    identifiers = {revision, f"path-{path}", f"url-{url}"}
    with (
        tempfile.TemporaryDirectory() as directory_name,
        CmdMox(verify_on_exit=False) as cmd_mox,
    ):
        boundaries = install_boundaries(cmd_mox)
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
    assert_identifiers_excluded_from_records(
        run.metrics, run.traces, identifiers, subject="generated identifiers"
    )


#: How long the child reports nothing for before the bound must escalate.
ESCALATION_SECONDS = 1
#: A child that traps ``TERM``, records its process ID, and then waits.
UNKILLABLE_CHILD = """#!/usr/bin/env bash
trap '' TERM
printf '%s\\n' "$$" >"$1"
while :; do sleep 1; done
"""


def _launch_bound(
    command: cuprum.SafeCmd, timeout_seconds: int
) -> cuprum.CommandResult | None:
    """Run *command* under the gate's own bound and return its result.

    Parameters
    ----------
    command
        The allow-listed child the gate would run.
    timeout_seconds
        The bound the gate configures for this operation.

    Returns
    -------
    cuprum.CommandResult | None
        The completed command, or ``None`` when the bound elapsed.
    """
    return commands.run_bounded_sync(command, timeout_seconds=timeout_seconds)


def _process_state(pid: int) -> str | None:
    """Return the process state letter, or ``None`` once the pid is gone.

    Parameters
    ----------
    pid
        Process identifier the probe child recorded for itself.

    Returns
    -------
    str | None
        The one-letter state from ``ps``, or ``None`` when no such process
        remains, which is how the escalation's reap is observed.
    """
    probe = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed probe argv.
        ["/usr/bin/ps", "-o", "stat=", "-p", str(pid)],
        capture_output=True,
        check=False,
        text=True,
    )
    return probe.stdout.strip() or None


def test_bounded_run_escalates_against_a_child_that_ignores_sigterm(
    tmp_path: pathlib.Path,
) -> None:
    """Verify the admission bound terminates a child that ignores ``SIGTERM``.

    Notes
    -----
    GNU ``timeout --kill-after=1s`` escalated to ``SIGKILL``, and the port must
    escalate identically, because a wedged adapter would otherwise hang the
    release job rather than classify as a timeout. A cmd-mox shim cannot stand
    in for the child: it is a fixed script the test does not own, so the
    escalation is exercised directly against the helper with a real child that
    traps ``TERM`` and reports its own process ID.

    Contract invariants: the bound reports ``None`` rather than an exit status,
    the gate's classification of that is ``124``, and the child is gone once the
    call returns -- reaped, not merely signalled.
    """
    child = tmp_path / "stubborn"
    child.write_text(UNKILLABLE_CHILD, encoding="utf-8")
    child.chmod(0o755)
    pid_file = tmp_path / "pid"
    program = cuprum.Program(str(child))
    catalogue = cuprum.ProgramCatalogue(
        projects=[
            cuprum.ProjectSettings(
                name="bounded-run-probe",
                programs=(program,),
                documentation_locations=(),
                noise_rules=(),
            )
        ]
    )
    command = cuprum.sh.make(program, catalogue=catalogue)(str(pid_file))

    result = _launch_bound(command, ESCALATION_SECONDS)

    assert result is None, "a bound that elapsed must report no result, not a status"
    assert commands.classify_result(result) == 124, (
        "an elapsed bound must classify as the timeout status"
    )
    pid = int(pid_file.read_text(encoding="utf-8").strip())
    assert _process_state(pid) is None, (
        "the escalate-to-kill path must leave no surviving child"
    )
