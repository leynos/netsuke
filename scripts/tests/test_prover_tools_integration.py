"""Exercise the pinned Verus CLI without installing a real verifier.

The suite drives the pinned ``rust-prover-tools`` CLI, and the repository's
``make verus`` target on top of it, against a fake verifier. That combination
is what lets the test run anywhere: the CLI's real batching, ordering and
failure-propagation logic is exercised, while the Verus binary that would need
a large platform-specific installation is replaced by a script that records the
arguments it receives. The separate ``tests/prover_tools_verus_e2e_tests.rs``
suite covers the same paths against the real pinned verifier.
"""

import os
import pathlib  # ruff: ignore[typing-only-standard-library-import] - Hypothesis resolves test annotations when it wraps them.
import shutil

import pytest
from hypothesis import HealthCheck, given, settings
from hypothesis import strategies as st
from prover_tools_integration_support import (
    REPOSITORY_ROOT,
    invocations,
    make_proofs,
    proof_for_invocation,
    run_command,
    run_make_verus,
    run_verus,
)


def test_pinned_cli_runs_repeated_proofs_in_order(tmp_path: pathlib.Path) -> None:
    """The real CLI invokes the fake verifier once per path, in order."""
    proofs = make_proofs(tmp_path)
    result = run_verus(tmp_path, proofs)
    invocation_log = tmp_path / "verus-invocations.jsonl"

    assert result.returncode == 0, result.stderr
    assert [
        proof_for_invocation(invocation, proofs)
        for invocation in invocations(invocation_log)
    ] == proofs, "the pinned CLI must execute all proof files in order"


@pytest.mark.parametrize("failed_index", [0, 1, 2], ids=["first", "middle", "last"])
def test_pinned_cli_stops_at_failure_and_preserves_verus_status(
    tmp_path: pathlib.Path, failed_index: int
) -> None:
    """Each failing position preserves status and prevents later runs."""
    proofs = make_proofs(tmp_path)
    result = run_verus(
        tmp_path,
        proofs,
        environment={"VERUS_FAIL_PROOF": str(proofs[failed_index])},
    )
    invocation_log = tmp_path / "verus-invocations.jsonl"
    completed_proofs = [
        proof_for_invocation(invocation, proofs)
        for invocation in invocations(invocation_log)
    ]

    assert result.returncode == 42, "the Verus failure status must be preserved"
    assert completed_proofs == proofs[: failed_index + 1], (
        "the pinned CLI must stop immediately after the failing proof"
    )
    if failed_index:
        assert "Proved before the failure:" in result.stdout + result.stderr, (
            "the CLI must report successful proofs before the failure"
        )
        for proof in proofs[:failed_index]:
            assert str(proof) in result.stdout + result.stderr, (
                f"the partial-success diagnostic must include {proof}"
            )


def test_pinned_cli_rejects_a_missing_proof_before_execution(
    tmp_path: pathlib.Path,
) -> None:
    """A missing input fails before the fake verifier runs."""
    missing_proof = tmp_path / "missing.rs"
    result = run_verus(tmp_path, [missing_proof])
    invocation_log = tmp_path / "verus-invocations.jsonl"

    assert result.returncode != 0, "a missing proof path must fail"
    assert invocations(invocation_log) == [], (
        "the pinned CLI must reject missing inputs before execution"
    )


def test_pinned_cli_uses_environment_proof_selection_as_a_fallback(
    tmp_path: pathlib.Path,
) -> None:
    """The documented environment fallback reaches the fake verifier."""
    proof = make_proofs(tmp_path)[0]
    result = run_verus(
        tmp_path,
        [],
        environment={"VERUS_PROOF_FILE": str(proof)},
    )
    invocation_log = tmp_path / "verus-invocations.jsonl"

    assert result.returncode == 0, result.stderr
    assert proof_for_invocation(invocations(invocation_log)[0], [proof]) == proof, (
        "VERUS_PROOF_FILE must select the proof when CLI flags are absent"
    )


def test_cli_proof_selection_overrides_environment_fallback(
    tmp_path: pathlib.Path,
) -> None:
    """An explicit path takes precedence over the environment fallback."""
    environment_proof, cli_proof = make_proofs(tmp_path)[:2]
    result = run_verus(
        tmp_path,
        [cli_proof],
        environment={"VERUS_PROOF_FILE": str(environment_proof)},
    )
    invocation_log = tmp_path / "verus-invocations.jsonl"

    assert result.returncode == 0, result.stderr
    calls = invocations(invocation_log)
    assert len(calls) == 1, "CLI selection must run exactly one proof"
    assert (
        proof_for_invocation(calls[0], [environment_proof, cli_proof]) == cli_proof
    ), "an explicit proof-file flag must override VERUS_PROOF_FILE"


def test_make_verus_default_invocation_names_the_canonical_proof() -> None:
    """The target's documented default proof is what an unadorned run requests."""
    make = shutil.which("make")
    assert make is not None, "the target contract test requires make"
    # An inherited override would change the recipe this test is inspecting.
    child_environment = os.environ.copy()
    for variable in ("VERUS_FLAGS", "MAKEFLAGS"):
        child_environment.pop(variable, None)
    result = run_command(
        [
            make,
            "--no-print-directory",
            "-n",
            "-f",
            "Makefile",
            "verus",
            "PROVER_TOOLS=prover-tools",
        ],
        cwd=REPOSITORY_ROOT,
        env=child_environment,
    )

    assert result.returncode == 0, result.stderr
    assert "verus run --proof-file verus/canonical_value/main.rs" in result.stdout, (
        f"the default proof must be the documented one, got {result.stdout!r}"
    )


def test_make_verus_runs_an_ordered_batch_through_the_target(
    tmp_path: pathlib.Path,
) -> None:
    """`make verus` passes an ordered batch to the verifier unchanged."""
    proofs = make_proofs(tmp_path)
    result = run_make_verus(tmp_path, proofs)
    calls = invocations(tmp_path / "verus-invocations.jsonl")

    assert result.returncode == 0, result.stderr
    assert [
        proof_for_invocation(invocation, proofs) for invocation in calls
    ] == proofs, "the target must execute every proof file in the order given"


def test_make_verus_propagates_a_verifier_failure(tmp_path: pathlib.Path) -> None:
    """A failing proof fails the target, which names the verifier's status.

    GNU make normalizes any failed recipe to its own exit status 2, so the
    verifier's code survives in the target's diagnostic rather than in make's
    status. Asserting both is what keeps a caller from having to guess which of
    the two carries the information.
    """
    proofs = make_proofs(tmp_path)
    result = run_make_verus(
        tmp_path, proofs, environment={"VERUS_FAIL_PROOF": str(proofs[1])}
    )
    calls = invocations(tmp_path / "verus-invocations.jsonl")

    assert result.returncode == 2, "a failed recipe is make status 2"
    assert "prover-tools: target=verus failed exit=42" in result.stderr, (
        "the target's diagnostic must name the verifier's status"
    )
    assert [proof_for_invocation(invocation, proofs) for invocation in calls] == proofs[
        :2
    ], "the target must stop after the failing proof"


@settings(
    max_examples=5,
    deadline=None,
    suppress_health_check=[HealthCheck.function_scoped_fixture],
)
@given(
    order=st.lists(
        st.integers(min_value=0, max_value=4), min_size=1, max_size=4, unique=True
    ),
    requested_position=st.integers(min_value=0, max_value=3),
)
def test_pinned_cli_stops_at_the_failing_proof_in_any_order(
    tmp_path: pathlib.Path, order: list[int], requested_position: int
) -> None:
    """Whatever the batch order, execution stops at and after the failure."""
    failed_position = requested_position % len(order)
    proofs = make_proofs(tmp_path, count=5)
    selected = [proofs[index] for index in order]
    result = run_verus(
        tmp_path,
        selected,
        environment={"VERUS_FAIL_PROOF": str(selected[failed_position])},
    )
    completed = [
        proof_for_invocation(invocation, selected)
        for invocation in invocations(tmp_path / "verus-invocations.jsonl")
    ]

    assert result.returncode == 42, "the Verus failure status must be preserved"
    assert completed == selected[: failed_position + 1], (
        "the pinned CLI must stop immediately after the failing proof"
    )
