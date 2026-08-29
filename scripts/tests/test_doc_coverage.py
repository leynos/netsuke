"""Test the documentation-coverage command-line interface."""

import argparse
import dataclasses
import json
import os
import pathlib

# The CLI's contract includes the process it launches, so these tests exercise
# it through a real child process.
# ruff: ignore[suspicious-subprocess-import] - the boundary is under test.
import subprocess
import sys
import textwrap
import typing as typ

import pytest
from conftest import SCRIPT_DIRECTORY

if typ.TYPE_CHECKING:
    import types


@dataclasses.dataclass(frozen=True, slots=True)
class ThresholdCase:
    """Define one in-process threshold-gating scenario."""

    threshold: str
    expected_code: int


@dataclasses.dataclass(frozen=True, slots=True)
class CliProcessCase:
    """Define one executable documentation-coverage CLI scenario."""

    threshold: str
    fails_adapter: bool
    expected_code: int


def coverage_rows(stdout: str) -> list[tuple[str, str, str]]:
    """Split the CLI breakdown table into label, counts, and percentage rows.

    Parameters
    ----------
    stdout
        Complete standard output captured from one CLI run.

    Returns
    -------
    list[tuple[str, str, str]]
        One entry per breakdown row, with the column padding removed.
    """
    rows: list[tuple[str, str, str]] = []
    for line in stdout.splitlines():
        fields = line.split()
        if len(fields) >= 3 and "/" in fields[-2]:
            rows.append((" ".join(fields[:-2]), fields[-2], fields[-1]))
    return rows


@pytest.fixture
def executable_cargo(tmp_path: pathlib.Path) -> tuple[pathlib.Path, pathlib.Path]:
    """Create a platform-safe Cargo executable for CLI process tests."""
    program = tmp_path / "fake_cargo.py"
    log_path = tmp_path / "cargo-calls.jsonl"
    program.write_text(
        textwrap.dedent(
            """\
            #!__PYTHON__
            import json
            import os
            import pathlib
            import sys

            args = sys.argv[1:]
            with pathlib.Path(os.environ["DOC_COVERAGE_CARGO_LOG"]).open("a") as log:
                print(json.dumps(args), file=log)
            if os.environ.get("DOC_COVERAGE_CARGO_FAILURE"):
                print("controlled cargo failure", file=sys.stderr)
                raise SystemExit(1)
            if args[1] == "metadata":
                print(
                    '{"packages": [{"id": "pkg:x:1.0.0", "name": "x", '
                    '"targets": [{"name": "x", "kind": ["lib"]}]}], '
                    '"workspace_members": ["pkg:x:1.0.0"]}'
                )
                raise SystemExit(0)
            output_path = pathlib.Path.cwd() / "target" / "doc" / "x.json"
            output_path.parent.mkdir(parents=True, exist_ok=True)
            output_path.write_text(
                '{"src/lib.rs": {"total": 10, "with_docs": 9}}', encoding="utf-8"
            )
            print(f'Generated output into "{output_path}"')
            """
        ).replace("__PYTHON__", sys.executable),
        encoding="utf-8",
    )
    if sys.platform == "win32":
        executable = tmp_path / "fake-cargo.cmd"
        executable.write_text(
            f'@echo off\r\n"{sys.executable}" "{program}" %*\r\n', encoding="utf-8"
        )
    else:
        executable = program
        executable.chmod(0o755)
    return executable, log_path


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(ThresholdCase(threshold="50", expected_code=0), id="above"),
        pytest.param(ThresholdCase(threshold="80", expected_code=1), id="below"),
    ],
)
def test_threshold_flips_exit_code(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    case: ThresholdCase,
) -> None:
    """Delegate measurement to the runner, then gate on the reported aggregate."""
    coverage = script.runner.Coverage(10, 6)
    monkeypatch.setattr(
        script.runner,
        "run_measurements",
        lambda _toolchain, _root: (coverage, []),
    )

    exit_code = script.main([
        "--toolchain",
        "nightly-x",
        "--manifest-root",
        str(tmp_path),
        "--threshold",
        case.threshold,
    ])

    assert exit_code == case.expected_code, (
        f"60% coverage against a {case.threshold}% threshold "
        f"must exit {case.expected_code}"
    )


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(
            CliProcessCase(threshold="80", fails_adapter=False, expected_code=0),
            id="passing-threshold",
        ),
        pytest.param(
            CliProcessCase(threshold="95", fails_adapter=False, expected_code=1),
            id="failing-threshold",
        ),
        pytest.param(
            CliProcessCase(threshold="80", fails_adapter=True, expected_code=2),
            id="adapter-failure",
        ),
    ],
)
def test_cli_process_uses_configured_cargo_adapter(
    executable_cargo: tuple[pathlib.Path, pathlib.Path],
    tmp_path: pathlib.Path,
    case: CliProcessCase,
) -> None:
    """Run the CLI against a controlled Cargo executable in a child process."""
    cargo_executable, log_path = executable_cargo
    environment = os.environ | {
        "CARGO": str(cargo_executable),
        "DOC_COVERAGE_CARGO_LOG": str(log_path),
    }
    if case.fails_adapter:
        environment["DOC_COVERAGE_CARGO_FAILURE"] = "1"
    # The argv is composed here from the interpreter and the fixture executable.
    # ruff: ignore[subprocess-without-shell-equals-true] - shell is False.
    result = subprocess.run(
        [
            sys.executable,
            str(SCRIPT_DIRECTORY / "doc-coverage.py"),
            "--manifest-root",
            str(tmp_path),
            "--toolchain",
            "nightly-subprocess",
            "--threshold",
            case.threshold,
        ],
        cwd=tmp_path,
        capture_output=True,
        check=False,
        env=environment,
        text=True,
    )

    assert result.returncode == case.expected_code, (
        f"expected exit {case.expected_code}, got {result.returncode}: {result.stderr}"
    )
    calls = [
        json.loads(line) for line in log_path.read_text(encoding="utf-8").splitlines()
    ]
    assert calls[0] == [
        "+nightly-subprocess",
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
    ], "the CLI must discover targets through cargo metadata on the given toolchain"
    if case.fails_adapter:
        assert result.stderr == (
            "error: cargo metadata failed: controlled cargo failure\n\n"
        ), "a failing Cargo adapter must surface as the CLI's measurement error"
        return

    assert calls[1][:5] == [
        "+nightly-subprocess",
        "rustdoc",
        "-p",
        "x",
        "--lib",
    ], "the discovered library target must be documented on the given toolchain"
    assert coverage_rows(result.stdout) == [
        ("x lib", "9/10", "90.00%"),
        ("aggregate", "9/10", "90.00%"),
    ], "the breakdown must report the measured target and the aggregate"
    trailer = result.stdout.splitlines()[2:]
    if case.expected_code == 0:
        assert trailer == [
            "ok: doc-comment coverage 90.00% meets the 80.00% threshold."
        ], "a passing run must confirm the threshold on standard output"
    else:
        assert trailer == [], "a failing run must report only the breakdown on stdout"
        assert result.stderr == (
            "doc-comment coverage 90.00% is below the 95.00% threshold; document "
            "the lowest-coverage targets listed above and re-run `make doc-coverage`.\n"
        ), "a failing run must explain the shortfall on standard error"


def test_missing_cargo_maps_to_measurement_error(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Map a missing Cargo executable to the established CLI failure exit."""
    message = "cargo: not found"

    def fail(_argv: list[str], **_kwargs: object) -> typ.NoReturn:
        """Raise the configured Cargo executable error."""
        raise OSError(message)

    monkeypatch.setattr(script.runner.doc_coverage_cargo.subprocess, "run", fail)
    monkeypatch.chdir(tmp_path)

    assert script.main(["--toolchain", "nightly-x"]) == 2, (
        "an unrunnable Cargo executable must exit with the measurement-error code"
    )


def test_toolchain_override_reaches_every_cargo_call(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Thread ``--toolchain`` through the ``+channel`` selector for every call."""
    calls: list[list[str]] = []
    metadata = (
        '{"packages": [{"id": "pkg:x:1.0.0", "name": "x", '
        '"targets": [{"name": "x", "kind": ["lib"]}]}], '
        '"workspace_members": ["pkg:x:1.0.0"]}'
    )

    def run(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
        """Record a Cargo call and return a minimal successful response."""
        calls.append(argv)
        if "metadata" in argv:
            return subprocess.CompletedProcess(argv, 0, metadata, "")
        manifest_root = pathlib.Path(typ.cast("pathlib.Path", kwargs["cwd"]))
        output_path = manifest_root / "target" / "doc" / "x.json"
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(
            '{"src/lib.rs": {"total": 10, "with_docs": 10}}',
            encoding="utf-8",
        )
        return subprocess.CompletedProcess(
            argv,
            0,
            f'Generated output into "{output_path}"\n',
            "",
        )

    monkeypatch.setattr(script.runner.doc_coverage_cargo.subprocess, "run", run)
    monkeypatch.chdir(tmp_path)

    script.main(["--toolchain", "nightly-custom", "--threshold", "0"])

    assert calls, "the CLI must invoke cargo at least once"
    assert all(argv[1] == "+nightly-custom" for argv in calls), (
        f"toolchain selector not threaded through every call: {calls!r}"
    )


@pytest.mark.parametrize(
    "value",
    [
        pytest.param("101", id="above-range"),
        pytest.param("-1", id="below-range"),
        pytest.param("nan", id="not-a-number-value"),
        pytest.param("not-a-number", id="unparsable"),
    ],
)
def test_parse_threshold_rejects_invalid_values(
    script: types.ModuleType, value: str
) -> None:
    """Reject thresholds outside [0, 100] and non-numbers as argument errors."""
    with pytest.raises(argparse.ArgumentTypeError):
        script.parse_threshold(value)


def test_label_names_libraries_and_binaries(script: types.ModuleType) -> None:
    """Distinguish library and named binary targets in breakdown labels."""
    doc_target = script.runner.DocTarget
    labels = (
        script.label(doc_target("netsuke", "lib", None)),
        script.label(doc_target("netsuke", "bin", "netsuke-bin")),
    )

    assert labels == ("netsuke lib", "netsuke bin (netsuke-bin)"), (
        "binary rows must be qualified by target name while library rows are not"
    )


def test_main_translates_runner_failure_to_exit_two(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Translate a runner failure into the established CLI diagnostic and exit code."""
    message = "runner measurement failed"

    def fail(_toolchain: str, _manifest_root: pathlib.Path) -> typ.NoReturn:
        """Raise the configured runner failure."""
        raise RuntimeError(message)

    monkeypatch.setattr(script.runner, "run_measurements", fail)

    exit_code = script.main([
        "--toolchain",
        "nightly-x",
        "--manifest-root",
        str(tmp_path),
    ])

    assert (exit_code, capsys.readouterr().err) == (2, f"error: {message}\n"), (
        "a runner failure must exit 2 and report the cause on standard error"
    )
