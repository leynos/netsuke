"""Exercise the pinned Verus CLI without installing a real verifier."""

import json
import os
import pathlib
import shlex
import stat
import subprocess  # ruff: ignore[suspicious-subprocess-import] - test CLI boundary.
import sys
import textwrap

import pytest

REPOSITORY_ROOT = pathlib.Path(__file__).resolve().parents[2]


def _write_fake_verus(tmp_path: pathlib.Path) -> tuple[pathlib.Path, pathlib.Path]:
    """Create a verifier that records argv and can fail on a selected proof."""
    executable = tmp_path / "fake-verus"
    invocation_log = tmp_path / "verus-invocations.jsonl"
    source = f"#!{sys.executable}\n" + textwrap.dedent(
        """\
        import json
        import os
        import pathlib
        import sys

        if sys.argv[1:] == ["--version"]:
            print("Verus\\nToolchain: nightly-2025-01-01")
            raise SystemExit(0)
        log = pathlib.Path(os.environ["VERUS_INVOCATION_LOG"])
        with log.open("a", encoding="utf-8") as stream:
            stream.write(json.dumps(sys.argv[1:]) + "\\n")
        if os.environ.get("VERUS_FAIL_PROOF") in sys.argv[1:]:
            raise SystemExit(int(os.environ.get("VERUS_FAIL_STATUS", "42")))
        """
    )
    executable.write_text(source, encoding="utf-8")
    executable.chmod(executable.stat().st_mode | stat.S_IXUSR)
    return executable, invocation_log


def _make_proofs(tmp_path: pathlib.Path) -> list[pathlib.Path]:
    """Create three existing files for ordered proof-run assertions."""
    proofs = [
        tmp_path / "proof one.rs",
        tmp_path / "proof two.rs",
        tmp_path / "proof three.rs",
    ]
    for proof in proofs:
        proof.write_text("// Fake verifier input.\n", encoding="utf-8")
    return proofs


def _run_verus(
    tmp_path: pathlib.Path,
    proofs: list[pathlib.Path],
    *,
    environment: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run the pinned CLI with tool setup disabled and a fake Verus binary."""
    command = shlex.split(os.environ["PROVER_TOOLS"])
    executable, invocation_log = _write_fake_verus(tmp_path)
    command.extend([
        "verus",
        "run",
        "--no-install-missing",
        "--no-ensure-toolchain",
        "--verus-bin",
        str(executable),
    ])
    for proof in proofs:
        command.extend(["--proof-file", str(proof)])

    child_environment = os.environ.copy()
    child_environment["VERUS_INVOCATION_LOG"] = str(invocation_log)
    if environment is not None:
        child_environment.update(environment)
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
        command,
        cwd=REPOSITORY_ROOT,
        env=child_environment,
        capture_output=True,
        check=False,
        shell=False,
        text=True,
    )


def _invocations(invocation_log: pathlib.Path) -> list[list[str]]:
    """Read the fake verifier's one-JSON-array-per-process log."""
    if not invocation_log.exists():
        return []
    return [json.loads(line) for line in invocation_log.read_text().splitlines()]


def _proof_for_invocation(
    invocation: list[str], proofs: list[pathlib.Path]
) -> pathlib.Path:
    """Return the one proof path passed to an invocation."""
    matches = [proof for proof in proofs if str(proof) in invocation]
    assert len(matches) == 1, f"expected one proof in argv, got {invocation!r}"
    return matches[0]


def test_pinned_cli_runs_repeated_proofs_in_order(tmp_path: pathlib.Path) -> None:
    """The real CLI invokes the fake verifier once per path, in order."""
    proofs = _make_proofs(tmp_path)
    result = _run_verus(tmp_path, proofs)
    invocation_log = tmp_path / "verus-invocations.jsonl"

    assert result.returncode == 0, result.stderr
    assert [
        _proof_for_invocation(invocation, proofs)
        for invocation in _invocations(invocation_log)
    ] == proofs, "the pinned CLI must execute all proof files in order"


@pytest.mark.parametrize("failed_index", [0, 1, 2], ids=["first", "middle", "last"])
def test_pinned_cli_stops_at_failure_and_preserves_verus_status(
    tmp_path: pathlib.Path, failed_index: int
) -> None:
    """Each failing position preserves status and prevents later runs."""
    proofs = _make_proofs(tmp_path)
    result = _run_verus(
        tmp_path,
        proofs,
        environment={"VERUS_FAIL_PROOF": str(proofs[failed_index])},
    )
    invocation_log = tmp_path / "verus-invocations.jsonl"
    invocations = _invocations(invocation_log)
    completed_proofs = [
        _proof_for_invocation(invocation, proofs) for invocation in invocations
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
    result = _run_verus(tmp_path, [missing_proof])
    invocation_log = tmp_path / "verus-invocations.jsonl"

    assert result.returncode != 0, "a missing proof path must fail"
    assert _invocations(invocation_log) == [], (
        "the pinned CLI must reject missing inputs before execution"
    )


def test_pinned_cli_uses_environment_proof_selection_as_a_fallback(
    tmp_path: pathlib.Path,
) -> None:
    """The documented environment fallback reaches the fake verifier."""
    proof = _make_proofs(tmp_path)[0]
    result = _run_verus(
        tmp_path,
        [],
        environment={"VERUS_PROOF_FILE": str(proof)},
    )
    invocation_log = tmp_path / "verus-invocations.jsonl"

    assert result.returncode == 0, result.stderr
    assert _proof_for_invocation(_invocations(invocation_log)[0], [proof]) == proof, (
        "VERUS_PROOF_FILE must select the proof when CLI flags are absent"
    )


def test_cli_proof_selection_overrides_environment_fallback(
    tmp_path: pathlib.Path,
) -> None:
    """An explicit path takes precedence over the environment fallback."""
    environment_proof, cli_proof = _make_proofs(tmp_path)[:2]
    result = _run_verus(
        tmp_path,
        [cli_proof],
        environment={"VERUS_PROOF_FILE": str(environment_proof)},
    )
    invocation_log = tmp_path / "verus-invocations.jsonl"

    assert result.returncode == 0, result.stderr
    invocations = _invocations(invocation_log)
    assert len(invocations) == 1, "CLI selection must run exactly one proof"
    assert (
        _proof_for_invocation(invocations[0], [environment_proof, cli_proof])
        == cli_proof
    ), "an explicit proof-file flag must override VERUS_PROOF_FILE"
