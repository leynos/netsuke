"""Exercise the pinned Verus CLI without installing a real verifier."""

import json
import os
import pathlib
import shlex
import shutil
import stat
import subprocess  # ruff: ignore[suspicious-subprocess-import] - test CLI boundary.
import sys
import textwrap

import pytest
from hypothesis import HealthCheck, given, settings
from hypothesis import strategies as st

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


def _make_proofs(tmp_path: pathlib.Path, count: int = 3) -> list[pathlib.Path]:
    """Create `count` existing files for ordered proof-run assertions."""
    names = ["one", "two", "three", "four", "five"]
    proofs = [tmp_path / f"proof {names[index]}.rs" for index in range(count)]
    for proof in proofs:
        proof.write_text("// Fake verifier input.\n", encoding="utf-8")
    return proofs


def _write_prover_tools_shim(tmp_path: pathlib.Path) -> pathlib.Path:
    """Create a single-word stand-in for the multi-word PROVER_TOOLS command.

    `make` substitutes `PROVER_TOOLS` into a recipe unquoted, so passing the
    real command as a make override would re-split and re-expand it. A shim
    that execs the command it reads from the environment keeps the override a
    single path while still driving the pinned CLI.

    Returns
    -------
    pathlib.Path
        The executable shim to pass as ``PROVER_TOOLS``.
    """
    shim = tmp_path / "prover-tools-shim"
    shim.write_text(
        f"#!{sys.executable}\n"
        + textwrap.dedent(
            """\
            import os
            import shlex
            import subprocess
            import sys

            argv = shlex.split(os.environ["PROVER_TOOLS_REAL"]) + sys.argv[1:]
            raise SystemExit(
                subprocess.run(argv, check=False, shell=False).returncode
            )
            """
        ),
        encoding="utf-8",
    )
    shim.chmod(shim.stat().st_mode | stat.S_IXUSR)
    return shim


def _run_verus(
    tmp_path: pathlib.Path,
    proofs: list[pathlib.Path],
    *,
    environment: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run the pinned CLI with tool setup disabled and a fake Verus binary."""
    command = shlex.split(os.environ["PROVER_TOOLS"])
    executable, invocation_log = _write_fake_verus(tmp_path)
    # Several examples may share one directory, so each run starts from an
    # empty log rather than inheriting the previous run's invocations.
    invocation_log.unlink(missing_ok=True)
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


def _run_make_verus(
    tmp_path: pathlib.Path,
    proofs: list[pathlib.Path],
    *,
    environment: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run the repository's `verus` target against the fake verifier.

    The target, not just the CLI, is under test here: it is what CI and
    maintainers invoke, and its variable plumbing is only exercised by
    expanding it for real.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The finished ``make`` process, with output captured.
    """
    make = shutil.which("make")
    assert make is not None, "the target contract test requires make"
    executable, invocation_log = _write_fake_verus(tmp_path)
    invocation_log.unlink(missing_ok=True)
    flags = shlex.join([
        "--no-install-missing",
        "--no-ensure-toolchain",
        "--verus-bin",
        str(executable),
        *(item for proof in proofs for item in ("--proof-file", str(proof))),
    ])
    child_environment = os.environ.copy()
    child_environment["PROVER_TOOLS_REAL"] = os.environ["PROVER_TOOLS"]
    child_environment["VERUS_INVOCATION_LOG"] = str(invocation_log)
    if environment is not None:
        child_environment.update(environment)
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
        [
            make,
            "--no-print-directory",
            "-f",
            "Makefile",
            "verus",
            f"PROVER_TOOLS={_write_prover_tools_shim(tmp_path)}",
            f"VERUS_FLAGS={flags}",
        ],
        cwd=REPOSITORY_ROOT,
        env=child_environment,
        capture_output=True,
        check=False,
        shell=False,
        text=True,
    )


def test_make_verus_default_invocation_names_the_canonical_proof() -> None:
    """The target's documented default proof is what an unadorned run requests."""
    make = shutil.which("make")
    assert make is not None, "the target contract test requires make"
    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
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
        capture_output=True,
        check=False,
        shell=False,
        text=True,
    )

    assert result.returncode == 0, result.stderr
    assert "verus run --proof-file verus/canonical_value/main.rs" in result.stdout, (
        f"the default proof must be the documented one, got {result.stdout!r}"
    )


def test_make_verus_runs_an_ordered_batch_through_the_target(
    tmp_path: pathlib.Path,
) -> None:
    """`make verus` passes an ordered batch to the verifier unchanged."""
    proofs = _make_proofs(tmp_path)
    result = _run_make_verus(tmp_path, proofs)
    invocations = _invocations(tmp_path / "verus-invocations.jsonl")

    assert result.returncode == 0, result.stderr
    assert [
        _proof_for_invocation(invocation, proofs) for invocation in invocations
    ] == proofs, "the target must execute every proof file in the order given"


def test_make_verus_propagates_a_verifier_failure(tmp_path: pathlib.Path) -> None:
    """A failing proof fails the target, which names the verifier's status.

    GNU make normalizes any failed recipe to its own exit status 2, so the
    verifier's code survives in the target's diagnostic rather than in make's
    status. Asserting both is what keeps a caller from having to guess which of
    the two carries the information.
    """
    proofs = _make_proofs(tmp_path)
    result = _run_make_verus(
        tmp_path, proofs, environment={"VERUS_FAIL_PROOF": str(proofs[1])}
    )
    invocations = _invocations(tmp_path / "verus-invocations.jsonl")

    assert result.returncode == 2, "a failed recipe is make status 2"
    assert "prover-tools: target=verus failed exit=42" in result.stderr, (
        "the target's diagnostic must name the verifier's status"
    )
    assert [
        _proof_for_invocation(invocation, proofs) for invocation in invocations
    ] == proofs[:2], "the target must stop after the failing proof"


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
    proofs = _make_proofs(tmp_path, count=5)
    selected = [proofs[index] for index in order]
    result = _run_verus(
        tmp_path,
        selected,
        environment={"VERUS_FAIL_PROOF": str(selected[failed_position])},
    )
    completed = [
        _proof_for_invocation(invocation, selected)
        for invocation in _invocations(tmp_path / "verus-invocations.jsonl")
    ]

    assert result.returncode == 42, "the Verus failure status must be preserved"
    assert completed == selected[: failed_position + 1], (
        "the pinned CLI must stop immediately after the failing proof"
    )
