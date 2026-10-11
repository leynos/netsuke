"""Exercise the Kani metrics helper through its API and workflow CLI."""

import dataclasses
import json
import os
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the CLI is exercised as a child.
import sys
import typing as typ

import pytest
from workflow_loading import MAKEFILE_PATH, REPO_ROOT

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

sys.path.insert(0, str(REPO_ROOT / "scripts"))

import kani_smoke_metrics as metrics
from kani_smoke_metrics_contract import (
    parse_metrics,
    validate_metrics,
)

SCRIPT = REPO_ROOT / "scripts" / "kani_smoke_metrics.py"
SCOPE_SCRIPT = REPO_ROOT / "scripts" / "kani_proof_scope.py"


@dataclasses.dataclass(frozen=True, slots=True)
class AcceptanceCase:
    """Carry one workflow decision and its expected metric labels."""

    decision: str
    changes: dict[str, str]
    expected_outcome: str
    expected_cache: str


def _inputs(**changes: str) -> metrics.WorkflowInputs:
    """Return one successful proof run with selected workflow fields changed."""
    values = {
        "event_name": "pull_request",
        "job_status": "success",
        "scope_outcome": "success",
        "run_proofs": "true",
        "cache_outcome": "success",
        "cache_hit": "true",
        "proofs_outcome": "success",
        "mutations_outcome": "success",
    }
    values.update(changes)
    return metrics.WorkflowInputs(**values)


def _run_finalizer(
    tmp_path: Path,
    *,
    decision: object,
    changes: dict[str, str] | None = None,
    decision_text: str | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run the finalizer subprocess using only its documented environment inputs."""
    state_directory = tmp_path / "state"
    state_directory.mkdir()
    (state_directory / metrics.MARK_START_FILE).write_text("100\n", encoding="ascii")
    (state_directory / metrics.MARK_END_FILE).write_text(
        "3000000100\n", encoding="ascii"
    )
    decision_file = tmp_path / "decision.json"
    if decision_text is None:
        decision_file.write_text(json.dumps({"decision": decision}), encoding="utf-8")
    else:
        decision_file.write_text(decision_text, encoding="utf-8")
    values = {
        "event_name": "pull_request",
        "job_status": "success",
        "scope_outcome": "success",
        "run_proofs": "true",
        "cache_outcome": "success",
        "cache_hit": "true",
        "proofs_outcome": "success",
        "mutations_outcome": "success",
        "state_directory": str(state_directory),
        "decision_file": str(decision_file),
        "metrics_file": str(tmp_path / "metrics.jsonl"),
    }
    if changes:
        values.update(changes)
    environment = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith("INPUT_")
        and key not in {"GITHUB_OUTPUT", "GITHUB_STEP_SUMMARY"}
    }
    environment.update({f"INPUT_{key.upper()}": value for key, value in values.items()})
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
        [sys.executable, str(SCRIPT), "finalize"],
        env=environment,
        capture_output=True,
        text=True,
        check=False,
    )


@pytest.mark.parametrize(
    "case",
    [
        AcceptanceCase("in_scope", {}, "success", "hit"),
        AcceptanceCase(
            "out_of_scope",
            {
                "run_proofs": "false",
                "proofs_outcome": "skipped",
                "mutations_outcome": "skipped",
                "cache_outcome": "skipped",
            },
            "skipped",
            "not_attempted",
        ),
        AcceptanceCase("unreadable_diff", {}, "success", "hit"),
        AcceptanceCase(
            "invalid_scope",
            {
                "job_status": "failure",
                "scope_outcome": "failure",
                "run_proofs": "",
                "proofs_outcome": "skipped",
                "mutations_outcome": "skipped",
            },
            "scope_failure",
            "hit",
        ),
        AcceptanceCase(
            "in_scope",
            {"job_status": "failure", "proofs_outcome": "failure"},
            "proof_failure",
            "hit",
        ),
    ],
    ids=["run", "skip", "unreadable-diff", "invalid-scope", "proof-failure"],
)
def test_cli_emits_acceptance_metrics(
    tmp_path: Path,
    case: AcceptanceCase,
) -> None:
    """Emit one bounded run and cache record for each required path."""
    completed = _run_finalizer(tmp_path, decision=case.decision, changes=case.changes)
    assert completed.returncode == 0, completed.stderr
    records = parse_metrics((tmp_path / "metrics.jsonl").read_text().splitlines())
    validate_metrics(records)
    assert records[0]["labels"] == {
        "event": "pull_request",
        "decision": case.decision,
        "outcome": case.expected_outcome,
    }, f"the run labels must preserve only the fixed case values: {records[0]}"
    assert records[1]["labels"] == {"result": case.expected_cache}, (
        f"the cache outcome must follow restore semantics: {records[1]}"
    )
    if case.changes.get("run_proofs", "true") == "true":
        assert records[2]["value"] == 3, (
            "the duration must cover the stored 3-second interval"
        )
    else:
        assert len(records) == 2, "a path that did not select proofs must omit duration"


@pytest.mark.parametrize(
    ("cache_outcome", "cache_hit", "expected"),
    [
        ("success", "true", "hit"),
        ("success", "false", "miss"),
        ("skipped", "true", "not_attempted"),
        ("failure", "true", "unknown"),
        ("success", "unexpected", "miss"),
    ],
)
def test_cache_classification_uses_only_reliable_action_outputs(
    cache_outcome: str, cache_hit: str, expected: str
) -> None:
    """Reserve hit/miss for a successful restore action's exposed output."""
    assert metrics.classify_cache(cache_outcome, cache_hit) == expected, (
        f"{cache_outcome}/{cache_hit} must map to {expected}"
    )


def test_classification_precedence_is_stable() -> None:
    """Apply cancellation, scope, skip, proof and mutation outcomes in order."""
    cases = [
        (_inputs(job_status="cancelled", scope_outcome="failure"), "cancelled"),
        (_inputs(scope_outcome="failure", run_proofs="false"), "scope_failure"),
        (_inputs(run_proofs="false", proofs_outcome="failure"), "skipped"),
        (_inputs(run_proofs="sometimes"), "invalid_decision"),
        (
            _inputs(proofs_outcome="failure", mutations_outcome="failure"),
            "proof_failure",
        ),
        (
            _inputs(job_status="failure", proofs_outcome="skipped"),
            "proof_not_run",
        ),
        (
            _inputs(job_status="failure", mutations_outcome="failure"),
            "mutation_failure",
        ),
        (_inputs(job_status="failure"), "job_failure"),
        (_inputs(), "success"),
        (_inputs(job_status="timed_out"), "unknown"),
    ]
    for inputs, expected in cases:
        assert metrics.classify_outcome(inputs) == expected, (
            f"the classifier must select {expected} for {inputs}"
        )


def test_unrecognized_event_and_decision_are_collapsed() -> None:
    """Keep arbitrary event and decision text out of metric labels."""
    records = metrics.metrics_records(
        "src/private.rs",
        _inputs(event_name="run-42:https://host/path"),
        2,
        1,
    )
    validate_metrics(records)
    assert records[0]["labels"] == {
        "event": "other",
        "decision": "unknown",
        "outcome": "success",
    }, f"raw event and decision inputs must collapse to bounded labels: {records[0]}"
    assert records[2]["value"] == 0, "negative monotonic deltas must be clamped"


def test_missing_end_mark_uses_the_injected_finalizer_clock(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Use finalizer time when a proof start exists but its end step did not run."""
    state_directory = tmp_path / "state"
    state_directory.mkdir()
    (state_directory / metrics.MARK_START_FILE).write_text("50\n", encoding="ascii")
    observed: list[dict[str, object]] = []
    dependencies = metrics.FinalizerDependencies(
        clock=lambda: 1_000_000_050,
        sink=lambda _path, records: observed.extend(records),
    )
    files = metrics.FinalizerFiles(
        str(state_directory), str(tmp_path / "decision"), str(tmp_path / "out")
    )
    finalize_inputs = _inputs()
    metrics.finalize(finalize_inputs, files, dependencies)
    assert observed[2]["value"] == 1, (
        "the injected clock must close the missing end mark"
    )
    assert capsys.readouterr().err == metrics.WARNING_END_FALLBACK + "\n", (
        "a missing end marker must use one fixed warning"
    )


def test_malformed_start_mark_omits_duration_with_fixed_warning(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Omit only duration when a timing mark cannot be parsed."""
    state_directory = tmp_path / "state"
    state_directory.mkdir()
    (state_directory / metrics.MARK_START_FILE).write_text("bad", encoding="ascii")
    records: list[dict[str, object]] = []
    metrics.finalize(
        _inputs(),
        metrics.FinalizerFiles(
            str(state_directory),
            str(tmp_path / "decision"),
            str(tmp_path / "out"),
        ),
        metrics.FinalizerDependencies(
            clock=lambda: 100, sink=lambda _path, values: records.extend(values)
        ),
    )
    assert len(records) == 2, (
        "a malformed start must leave counters while omitting duration"
    )
    assert capsys.readouterr().err == metrics.WARNING_DURATION + "\n", (
        "malformed timing data must not disclose its contents"
    )


def test_mark_commands_write_monotonic_values_in_the_state_directory(
    tmp_path: Path,
) -> None:
    """Expose separate start and end commands for workflow step placement."""
    state_directory = tmp_path / "state"
    environment = {
        **os.environ,
        "INPUT_STATE_DIRECTORY": str(state_directory),
    }
    for command, mark_name in (
        ("mark-start", metrics.MARK_START_FILE),
        ("mark-end", metrics.MARK_END_FILE),
    ):
        completed = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
            [sys.executable, str(SCRIPT), command],
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )
        assert completed.returncode == 0, completed.stderr
        value = int((state_directory / mark_name).read_text(encoding="ascii"))
        assert value >= 0, f"{command} must write a monotonic nanosecond value"


@pytest.mark.parametrize("mark", [metrics.mark_start, metrics.mark_end])
def test_mark_write_failure_warns_without_raising(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], mark: cabc.Callable[..., None]
) -> None:
    """Keep timing-file failures non-fatal to proof and mutation commands."""
    blocked_directory = tmp_path / "not-a-directory"
    blocked_directory.write_text("file", encoding="utf-8")
    mark(blocked_directory, clock=lambda: 123)
    assert capsys.readouterr().err == metrics.WARNING_MARK_WRITE + "\n", (
        "a mark write failure must produce one fixed warning"
    )


def test_metrics_sink_failure_has_one_fixed_error_and_nonzero_exit(
    tmp_path: Path,
) -> None:
    """Fail only the finalizer when the metrics artifact cannot be delivered."""
    blocked = tmp_path / "file"
    blocked.write_text("block", encoding="utf-8")
    completed = _run_finalizer(
        tmp_path,
        decision="in_scope",
        changes={"metrics_file": str(blocked / "metrics.jsonl")},
    )
    assert completed.returncode != 0, "a missing artifact must fail the finalizer"
    assert completed.stderr == metrics.ERROR_METRICS_WRITE + "\n", (
        "the finalizer must not disclose paths or raw filesystem errors"
    )


def test_uncertain_inputs_still_write_unknown_metrics(tmp_path: Path) -> None:
    """Preserve delivery when decision or outcome inputs cannot be classified."""
    completed = _run_finalizer(
        tmp_path,
        decision="ignored",
        decision_text="not-json",
        changes={
            "event_name": "workflow_dispatch:private",
            "job_status": "unrecognised",
            "run_proofs": "maybe",
            "proofs_outcome": "unavailable",
        },
    )
    assert completed.returncode == 0, completed.stderr
    records = parse_metrics((tmp_path / "metrics.jsonl").read_text().splitlines())
    validate_metrics(records)
    run_labels = records[0].get("labels")
    assert isinstance(run_labels, dict), "run metrics must carry labels"
    assert run_labels["event"] == "other", "events must remain bounded"
    assert run_labels["decision"] == "unknown", "bad records must collapse"
    assert run_labels["outcome"] == "invalid_decision", (
        "an invalid run-proofs output must remain distinguishable"
    )


def test_dependency_pins_match_the_makefile() -> None:
    """Keep both Kani scripts' pins aligned with test and typecheck tooling."""
    scripts = (SCOPE_SCRIPT, SCRIPT)
    pins_by_script = {
        script.name: [
            line.strip().strip('#", ')
            for line in script
            .read_text(encoding="utf-8")
            .split("# ///", 2)[1]
            .splitlines()
            if "==" in line
        ]
        for script in scripts
    }
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    pins = pins_by_script[SCOPE_SCRIPT.name]
    assert len(pins) == 2, f"expected the cuprum and cyclopts pins, found {pins}"
    assert pins_by_script[SCRIPT.name] == pins, (
        "the scope and metrics scripts must declare identical dependency pins"
    )
    workflow_recipes = makefile.split("test-workflow-contracts:", 1)[1].split(
        "test-windows-msi-release-rank:", 1
    )[0]
    workflow_contracts = workflow_recipes.split("\n\tPYTHONPATH=scripts", 1)[0]
    typecheck = makefile.split("typecheck-python:", 1)[1].split("\nmarkdownlint:", 1)[0]
    for pin in pins:
        package, version = pin.split("==", 1)
        assert workflow_contracts.count(f"--with '{pin}'") == 1, (
            f"the workflow contracts must run Kani with its declared pin {pin}"
        )
        assert f"--with '{package}>={version}," in typecheck, (
            f"`make typecheck-python` must support Kani's declared pin {pin}"
        )
