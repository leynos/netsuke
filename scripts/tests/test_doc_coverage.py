"""Test the documentation-coverage command-line interface."""

from __future__ import annotations

import argparse
import dataclasses
import json
import os
import pathlib
import subprocess
import sys
import textwrap
import typing as typ

import pytest
from conftest import SCRIPT_DIRECTORY

if typ.TYPE_CHECKING:
    import types


@dataclasses.dataclass(frozen=True)
class CliProcessCase:
    """Define one executable documentation-coverage CLI scenario."""

    threshold: str
    fails_adapter: bool
    expected_code: int


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


def test_threshold_flips_exit_code(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Pass above the threshold and fail below it."""
    coverage = script.runner.Coverage(10, 6)
    monkeypatch.setattr(
        script.runner,
        "run_measurements",
        lambda _toolchain, _root: (coverage, []),
    )
    monkeypatch.chdir(tmp_path)

    passing = script.main(["--toolchain", "nightly-x", "--threshold", "50"])
    failing = script.main(["--toolchain", "nightly-x", "--threshold", "80"])

    assert passing == 0
    assert failing == 1


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
    result = subprocess.run(  # noqa: S603 - executes the controlled fixture with shell disabled.
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

    assert result.returncode == case.expected_code
    calls = [
        json.loads(line) for line in log_path.read_text(encoding="utf-8").splitlines()
    ]
    assert calls[0] == [
        "+nightly-subprocess",
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
    ]
    if case.fails_adapter:
        assert (
            result.stderr
            == "error: cargo metadata failed: controlled cargo failure\n\n"
        )
        return

    assert calls[1][0] == "+nightly-subprocess"
    assert calls[1][1:5] == ["rustdoc", "-p", "x", "--lib"]
    assert "aggregate" in result.stdout
    assert "9/10" in result.stdout
    if case.expected_code == 0:
        assert (
            "ok: doc-comment coverage 90.00% meets the 80.00% threshold."
            in result.stdout
        )
    else:
        assert result.stderr == (
            "doc-comment coverage 90.00% is below the 95.00% threshold; document "
            "the lowest-coverage targets listed above and re-run `make doc-coverage`.\n"
        )


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

    assert script.main(["--toolchain", "nightly-x"]) == 2


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

    assert calls, "expected at least one cargo invocation"
    assert all(argv[1] == "+nightly-custom" for argv in calls), (
        f"toolchain selector not threaded through: {calls!r}"
    )


def test_parse_threshold_rejects_invalid_values(script: types.ModuleType) -> None:
    """Reject thresholds outside [0, 100] and non-numbers as argument errors."""
    for value in ["101", "-1", "not-a-number"]:
        with pytest.raises(argparse.ArgumentTypeError):
            script.parse_threshold(value)


def test_label_names_libraries_and_binaries(script: types.ModuleType) -> None:
    """Distinguish library and named binary targets in breakdown labels."""
    lib = script.DocTarget("netsuke", "lib", None)
    binary = script.DocTarget("netsuke", "bin", "netsuke-bin")

    assert script.label(lib) == "netsuke lib"
    assert script.label(binary) == "netsuke bin (netsuke-bin)"


def test_main_delegates_to_runner_measurements(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Delegate CLI measurement while retaining CLI reporting and exit policy."""
    expected = script.runner.Coverage(1, 1)
    monkeypatch.setattr(
        script.runner,
        "run_measurements",
        lambda _toolchain, _root: (expected, []),
    )

    assert (
        script.main(["--toolchain", "nightly-x", "--manifest-root", str(tmp_path)]) == 0
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

    assert (
        script.main(["--toolchain", "nightly-x", "--manifest-root", str(tmp_path)]) == 2
    )
    assert capsys.readouterr().err == f"error: {message}\n"
