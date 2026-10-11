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

import os
import pathlib
import sys
import tempfile
import time
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
    Boundaries,
    GateRun,
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
    bounding,
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


def _run_under_generated_identifiers(
    revision: str, path: str, url: str
) -> tuple[GateRun, Boundaries]:
    """Run the gate once with the given identifiers in its environment.

    A controller is built per example rather than taken from the fixture: the
    doubles must be registered before replay, and Hypothesis runs every example
    inside one test function, so one fixture instance cannot serve them all.

    Parameters
    ----------
    revision
        The canonical object ID the workflow exports and the queries must carry.
    path, url
        Generated identifiers carried in the child's environment only.

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
    """
    del run_id
    identifiers = {revision, f"path-{path}", f"url-{url}"}
    run, boundaries = _run_under_generated_identifiers(revision, path, url)

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
#: How much longer than ``ESCALATION_SECONDS + CANCEL_GRACE_SECONDS`` the whole
#: call may take before the bound has demonstrably failed. The allowance is
#: generous enough for a loaded machine to schedule the escalation, and far
#: below the overrun a surviving descendant that holds the captured pipes
#: produces: the port measured that at 120 seconds against a 2 second bound.
OVERRUN_ALLOWANCE_SECONDS = 10.0
#: A child that traps ``TERM``, records its own process ID, and then waits.
#:
#: The descendant is the load-bearing part. It inherits the child's streams --
#: including the pipe the bound captures -- and traps ``TERM`` as well, so it
#: keeps that pipe open after the direct child is signalled. A bound that
#: signals only the direct child therefore never reaches end-of-file on the
#: drain and overruns for as long as the descendant lives, which is the whole
#: defect this pair of assertions exists to catch. The probe starts the
#: descendant named by its second argument, and the descendant records its own
#: pid there, so the test can check the two cleanups separately.
UNKILLABLE_CHILD = """#!/usr/bin/env bash
trap '' TERM
printf '%s\\n' "$$" >"$1"
"$2" "$1.desc" &
while :; do sleep 1; done
"""

#: The descendant the probe starts. It too ignores ``TERM``, so the bound's
#: ``SIGKILL`` escalation is the only thing that can end it.
UNKILLABLE_DESCENDANT = """#!/usr/bin/env bash
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


def _process_exists(pid: int) -> bool:
    """Return whether the pid still names a live process.

    Parameters
    ----------
    pid
        Process identifier the probe child recorded for itself.

    Returns
    -------
    bool
        Whether the process is still there, which is how the escalation's reap
        is observed.

    Notes
    -----
    Signal ``0`` is the portable liveness question: it performs the permission
    and existence checks without delivering a signal, and raises
    :class:`ProcessLookupError` exactly when no such process remains. It
    replaces an earlier ``ps`` probe, which answered the same question but
    depended on a binary at a fixed path and on how that binary spells its
    output. The child is reaped by the call under test, so the zone where it
    could vanish between this check and the previous assertion belongs to that
    call rather than here.

    A process the kernel has reaped but whose identifier another process has
    since reused would be reported as alive. The window is microscopic and the
    mistake is in the safe direction: it fails this test rather than passing a
    broken one.
    """
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    return True


def _bounded_probe_command(
    child: pathlib.Path, pid_file: pathlib.Path
) -> cuprum.SafeCmd:
    """Build the allow-listed probe command the bound is exercised against.

    Parameters
    ----------
    child
        The probe script that traps ``TERM`` and starts a descendant.
    pid_file
        Where the child and its descendant record their process identifiers.

    Returns
    -------
    cuprum.SafeCmd
        The command: the pid file, then the descendant the probe must start.
    """
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
    descendant = child.with_name(f"{child.name}.desc")
    return cuprum.sh.make(program, catalogue=catalogue)(str(pid_file), str(descendant))


def _write_probe(child: pathlib.Path) -> None:
    """Write the unkillable probe and its descendant beside it, both executable.

    The descendant is written next to the probe under the ``.desc`` suffix the
    probe appends to its own path, so the two travel as a set.
    """
    child.write_text(UNKILLABLE_CHILD, encoding="utf-8")
    child.chmod(0o755)
    descendant = child.with_name(f"{child.name}.desc")
    descendant.write_text(UNKILLABLE_DESCENDANT, encoding="utf-8")
    descendant.chmod(0o755)


def _recorded_pid(pid_file: pathlib.Path) -> int:
    """Return the process identifier a probe recorded in *pid_file*."""
    return int(pid_file.read_text(encoding="utf-8").strip())


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
    _write_probe(child)
    pid_file = tmp_path / "pid"

    result = _launch_bound(_bounded_probe_command(child, pid_file), ESCALATION_SECONDS)

    assert result is None, "a bound that elapsed must report no result, not a status"
    assert commands.classify_result(result) == 124, (
        "an elapsed bound must classify as the timeout status"
    )
    assert not _process_exists(_recorded_pid(pid_file)), (
        "the escalate-to-kill path must leave no surviving child"
    )


def test_bounded_run_reaps_a_descendant_that_holds_the_captured_pipe(
    tmp_path: pathlib.Path,
) -> None:
    """Verify the bound reaches a descendant, within the bound, as GNU timeout did.

    Notes
    -----
    This is the regression test for the defect the shell's ``timeout`` hid.
    cuprum spawns without ``start_new_session`` and terminates only the direct
    child, so a descendant survives cancellation; worse, a survivor that
    inherited the captured stdout keeps the pipe open, the drain never reaches
    end-of-file, and a two-second bound overran by 120 seconds when measured.

    Both halves are asserted, because either alone would pass a broken bound.
    An elapsed bound with a live descendant is the old leak; a dead descendant
    that took four minutes to reach is the old overrun. The child and its
    descendant are checked separately: the direct child is reaped by
    ``process.wait()`` either way, so only the descendant distinguishes a
    group signal from a pid signal.

    A ``setsid()``-escaping descendant still escapes, which is a limitation the
    developers' guide records for the Kani scope wrapper as well; this probe
    stays in the group, as every ordinary adapter does.

    The bound is ``ESCALATION_SECONDS`` plus the helper's own cancel grace, and
    the elapsed check allows ``OVERRUN_ALLOWANCE_SECONDS`` on top for
    scheduling. The two are not in tension: measured, the call returns in
    ``bound + grace`` and the descendant is already gone.
    """
    child = tmp_path / "stubborn"
    _write_probe(child)
    pid_file = tmp_path / "pid"

    started = time.monotonic()
    result = _launch_bound(_bounded_probe_command(child, pid_file), ESCALATION_SECONDS)
    elapsed = time.monotonic() - started

    limit = (
        ESCALATION_SECONDS + bounding.CANCEL_GRACE_SECONDS + OVERRUN_ALLOWANCE_SECONDS
    )
    assert elapsed < limit, (
        f"a surviving descendant holding the captured pipe must not delay the "
        f"bound: {elapsed:.2f}s against a {limit:.2f}s limit"
    )
    assert result is None, "a bound that elapsed must report no result, not a status"
    assert commands.classify_result(result) == 124, (
        "an elapsed bound must classify as the timeout status"
    )
    descendant_pid = _recorded_pid(pid_file.with_name(f"{pid_file.name}.desc"))
    assert not _process_exists(descendant_pid), (
        "the group signal must reach a descendant the direct child started"
    )
