"""Substantive tests for the workspace Rustdoc doc-comment coverage gate.

``scripts/doc_coverage_cargo.py`` wraps ``cargo rustdoc --show-coverage`` and
``cargo metadata``; every test in this module replaces that subprocess boundary
with canned responses. The runner tests cover target discovery and aggregation,
while the CLI tests separately exercise threshold exits and diagnostic
translation.

The helper script cannot be imported by its file name (``doc-coverage.py``
contains a hyphen), so a fixture loads it through ``importlib`` under a
hyphen-free module name.
"""

from __future__ import annotations

import argparse
import dataclasses
import importlib.util
import json
import os
import pathlib
import subprocess
import sys
import textwrap
import typing as typ

import pytest

if typ.TYPE_CHECKING:
    import types

SCRIPT_DIRECTORY = pathlib.Path(__file__).resolve().parents[1]


def load_script_module(module_name: str, file_name: str) -> types.ModuleType:
    """Import one documentation-coverage module under its required name."""
    spec = importlib.util.spec_from_file_location(
        module_name, SCRIPT_DIRECTORY / file_name
    )
    assert spec is not None, "expected import setup to produce a module spec"
    assert spec.loader is not None, "expected module spec to provide a loader"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture(name="cargo")
def cargo_fixture() -> types.ModuleType:
    """Import the Cargo and Rustdoc adapter under its normal module name."""
    return load_script_module("doc_coverage_cargo", "doc_coverage_cargo.py")


@pytest.fixture(name="runner")
def runner_fixture(cargo: types.ModuleType) -> types.ModuleType:
    """Import the measurement coordinator under its normal module name."""
    return load_script_module("doc_coverage_runner", "doc_coverage_runner.py")


@pytest.fixture(name="script")
def script_fixture(runner: types.ModuleType) -> types.ModuleType:
    """Import ``doc-coverage.py`` under a loadable module name."""
    return load_script_module("doc_coverage_module", "doc-coverage.py")


def lib_target(name: str) -> dict:
    """Return one library target as ``cargo metadata`` reports it."""
    return {"name": name, "kind": ["lib"]}


def bin_target(name: str) -> dict:
    """Return one binary target as ``cargo metadata`` reports it."""
    return {"name": name, "kind": ["bin"]}


def metadata_for(packages: list[dict]) -> dict:
    """Build the ``cargo metadata`` document the script consumes."""
    return {
        "packages": packages,
        "workspace_members": [package["id"] for package in packages],
    }


def single_library_metadata() -> str:
    """Return the ``cargo metadata`` JSON for one package with one library target.

    Returns
    -------
    str
        A metadata document describing the single ``x`` package with one ``lib``
        target, in the shape ``doc_targets`` consumes.
    """
    return (
        '{"packages": [{"id": "pkg:x:1.0.0", "name": "x", '
        '"targets": [{"name": "x", "kind": ["lib"]}]}], '
        '"workspace_members": ["pkg:x:1.0.0"]}'
    )


@dataclasses.dataclass(frozen=True)
class RustdocFailureCase:
    """Define one Rustdoc failure scenario for measurement integration tests.

    Parameters
    ----------
    output
        The mocked standard output from `cargo rustdoc`.
    returncode
        The mocked `cargo rustdoc` process exit code.
    diagnostic
        Text that must occur in the translated `RuntimeError`.
    """

    output: str
    returncode: int
    diagnostic: str


@dataclasses.dataclass(frozen=True)
class CoveragePayloadFailureCase:
    """Define one invalid Rustdoc coverage-payload scenario.

    Parameters
    ----------
    payload
        The mocked output from `cargo rustdoc`.
    diagnostic
        Text that must occur in the translated `RuntimeError`.
    """

    payload: str
    diagnostic: str


@dataclasses.dataclass(frozen=True)
class CliProcessCase:
    """Define one executable documentation-coverage CLI scenario."""

    threshold: str
    fails_adapter: bool
    expected_code: int


@dataclasses.dataclass(frozen=True)
class ReportedCoverageFileCase:
    """Define one generated Rustdoc coverage-file path scenario."""

    payload: str
    output_path: pathlib.Path
    reported_path: pathlib.Path | None
    expected: tuple[int, int]


@dataclasses.dataclass(frozen=True)
class FakeRustdocResult:
    """Define the simulated result of one ``cargo rustdoc`` invocation."""

    payload: str = "{}"
    returncode: int = 0
    output_path: pathlib.Path | None = None
    reported_path: pathlib.Path | None = None
    write_output: bool = True


class FakeCargo:
    """Stand-in for ``cargo metadata`` and ``cargo rustdoc`` invocations.

    Each call records its argv, answers metadata with ``metadata``, and writes
    each Rustdoc payload to the generated file path that Rustdoc reports.
    """

    def __init__(
        self,
        cargo: types.ModuleType,
        *,
        metadata: str = '{"packages": [], "workspace_members": []}',
        rustdoc: FakeRustdocResult = FakeRustdocResult(),
    ) -> None:
        self._cargo = cargo
        self.metadata_payload = metadata
        self.rustdoc = rustdoc
        self.calls: list[list[str]] = []

    def install(self, monkeypatch: pytest.MonkeyPatch) -> FakeCargo:
        """Replace the adapter's ``subprocess.run`` with this fake."""
        monkeypatch.setattr(self._cargo.subprocess, "run", self.run)
        return self

    def run(self, argv: list[str], **kwargs: object) -> FakeResult:
        """Answer one Cargo invocation from the canned payloads."""
        self.calls.append(argv)
        if "metadata" in argv:
            return FakeResult(0, self.metadata_payload)
        manifest_root = pathlib.Path(typ.cast("pathlib.Path", kwargs["cwd"]))
        package = argv[argv.index("-p") + 1].replace("-", "_")
        configured_output_path = self.rustdoc.output_path or (
            manifest_root / "target" / "doc" / f"{package}.json"
        )
        output_path = (
            configured_output_path
            if configured_output_path.is_absolute()
            else manifest_root / configured_output_path
        )
        if self.rustdoc.write_output:
            output_path.parent.mkdir(parents=True, exist_ok=True)
            output_path.write_text(self.rustdoc.payload, encoding="utf-8")
        reported_path = self.rustdoc.reported_path or output_path
        return FakeResult(
            self.rustdoc.returncode,
            f'Generated output into "{reported_path}"\n',
        )


class FakeCoverageAdapter:
    """Record runner calls while returning one target and its coverage result."""

    def __init__(self, calls: list[object], coverage: object) -> None:
        self._calls = calls
        self._coverage = coverage

    def load_metadata(
        self, toolchain: str, manifest_root: pathlib.Path
    ) -> dict[str, object]:
        """Return metadata containing the one target under test."""
        self._calls.append(("metadata", toolchain, manifest_root))
        return {
            "packages": [
                {
                    "id": "pkg:x:1.0.0",
                    "name": "x",
                    "targets": [{"name": "x", "kind": ["lib"]}],
                }
            ],
            "workspace_members": ["pkg:x:1.0.0"],
        }

    def measure(
        self, target: object, toolchain: str, manifest_root: pathlib.Path
    ) -> object:
        """Record the selected target and return fixed coverage."""
        self._calls.append((target, toolchain, manifest_root))
        return self._coverage


class FakeResult:
    """Minimal ``subprocess.CompletedProcess`` stand-in."""

    def __init__(self, returncode: int, stdout: str, stderr: str = "") -> None:
        self.returncode = returncode
        self.stdout = stdout
        self.stderr = stderr


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


def test_target_discovery_skips_non_doc_targets(runner: types.ModuleType) -> None:
    """Build scripts, tests, examples, and benches never enter the surface."""
    metadata = metadata_for(
        [
            {
                "id": "pkg:netsuke:0.1.0",
                "name": "netsuke",
                "targets": [
                    lib_target("netsuke"),
                    bin_target("netsuke-bin"),
                    bin_target("extra"),
                    {"name": "build-main", "kind": ["custom-build"]},
                    {"name": "integration", "kind": ["test"]},
                    {"name": "sample", "kind": ["example"]},
                    {"name": "benchmark", "kind": ["bench"]},
                ],
            }
        ]
    )

    targets = runner.doc_targets(metadata)

    assert [target.kind for target in targets] == ["lib", "bin", "bin"]
    assert {target.name for target in targets if target.kind == "bin"} == {
        "netsuke-bin",
        "extra",
    }


def test_target_discovery_excludes_outside_workspace(runner: types.ModuleType) -> None:
    """Dependency crates outside ``workspace_members`` never get measured."""
    member = {
        "id": "pkg:member:0.1.0",
        "name": "member",
        "targets": [lib_target("member")],
    }
    dependency = {
        "id": "pkg:dependency:0.1.0",
        "name": "dependency",
        "targets": [lib_target("dependency")],
    }
    metadata = {
        "packages": [member, dependency],
        "workspace_members": ["pkg:member:0.1.0"],
    }

    targets = runner.doc_targets(metadata)

    assert [target.package for target in targets] == ["member"]


def test_aggregation_sums_targets_and_reports_percentage(
    runner: types.ModuleType,
) -> None:
    """Aggregate totals roll per-target counts up and report the share."""
    first = runner.Coverage(10, 8)
    second = runner.Coverage(5, 5)

    combined = first + second

    assert combined.total == 15
    assert combined.with_docs == 13
    assert combined.percentage == pytest.approx(13 / 15 * 100)


def test_empty_run_is_complete_not_a_division_by_zero(runner: types.ModuleType) -> None:
    """A crate with no doc-able targets contributes an empty, complete run."""
    assert runner.Coverage(0, 0).percentage == 100.0


def test_threshold_flips_exit_code(
    script: types.ModuleType,
    cargo: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The gate passes above the threshold and fails below it."""
    metadata = (
        '{"packages": [{"id": "pkg:x:1.0.0", "name": "x", '
        '"targets": [{"name": "x", "kind": ["lib"]}]}], '
        '"workspace_members": ["pkg:x:1.0.0"]}'
    )
    rustdoc = '{"src/lib.rs": {"total": 10, "with_docs": 6}}'
    FakeCargo(
        cargo, metadata=metadata, rustdoc=FakeRustdocResult(payload=rustdoc)
    ).install(monkeypatch)
    monkeypatch.chdir(tmp_path)

    passing = script.main(["--toolchain", "nightly-x", "--threshold", "50"])
    failing = script.main(["--toolchain", "nightly-x", "--threshold", "80"])

    assert passing == 0
    assert failing == 1


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(CliProcessCase("80", False, 0), id="passing-threshold"),
        pytest.param(CliProcessCase("95", False, 1), id="failing-threshold"),
        pytest.param(CliProcessCase("80", True, 2), id="adapter-failure"),
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


def test_cargo_metadata_failure_aborts_the_run(
    cargo: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A failing cargo metadata run is a measurement error, not an empty gate."""

    def fail(_argv: list[str], **_kwargs: object) -> FakeResult:
        return FakeResult(1, "", "filtered diagnostics")

    monkeypatch.setattr(cargo.subprocess, "run", fail)
    monkeypatch.chdir(tmp_path)

    with pytest.raises(RuntimeError, match="cargo metadata failed"):
        cargo.CargoAdapter("cargo").load_metadata("nightly-x", tmp_path)


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(
            RustdocFailureCase(
                "not json at all",
                0,
                "did not emit coverage JSON",
            ),
            id="malformed-output",
        ),
        pytest.param(
            RustdocFailureCase(
                "{}",
                1,
                "cargo rustdoc failed for x",
            ),
            id="rustdoc-exit-failure",
        ),
    ],
)
def test_cargo_adapter_propagates_rustdoc_failure(
    cargo: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    case: RustdocFailureCase,
) -> None:
    """Propagate malformed output and non-zero rustdoc exits as measurement errors."""
    FakeCargo(
        cargo,
        metadata=single_library_metadata(),
        rustdoc=FakeRustdocResult(
            payload=case.output,
            returncode=case.returncode,
        ),
    ).install(monkeypatch)
    monkeypatch.chdir(tmp_path)

    with pytest.raises(RuntimeError, match=case.diagnostic):
        cargo.CargoAdapter("cargo").measure(
            cargo.DocTarget("x", "lib", None), "nightly-x", tmp_path
        )


def test_malformed_metadata_shape_is_a_measurement_error(
    runner: types.ModuleType,
) -> None:
    """Valid JSON without workspace keys is rejected, not a KeyError crash."""
    with pytest.raises(RuntimeError, match="lacks the workspace"):
        runner.doc_targets({"packages": []})


def test_target_without_kind_is_skipped(runner: types.ModuleType) -> None:
    """A target record missing its kind list simply contributes nothing."""
    metadata = metadata_for(
        [
            {
                "id": "pkg:x:0.1.0",
                "name": "x",
                "targets": [{"name": "mystery"}],
            }
        ]
    )

    assert runner.doc_targets(metadata) == []


def test_missing_cargo_maps_to_measurement_error(
    script: types.ModuleType,
    cargo: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """An OSError from Cargo exits 2 with a message, not a traceback."""

    def fail(_argv: list[str], **_kwargs: object) -> FakeResult:
        message = "cargo: not found"
        raise OSError(message)

    monkeypatch.setattr(cargo.subprocess, "run", fail)
    monkeypatch.chdir(tmp_path)

    code = script.main(["--toolchain", "nightly-x"])

    assert code == 2


def test_measure_maps_missing_cargo_to_measurement_error(
    cargo: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """An OSError from Cargo in ``measure()`` is a RuntimeError, not a traceback.

    ``test_missing_cargo_maps_to_measurement_error`` covers the same boundary
    through ``main()``, where the error also surfaces at ``load_metadata()``
    time. This test isolates the ``measure()`` translation branch after
    metadata loading has already succeeded, so the diagnostic is the rustdoc
    one rather than the metadata one.
    """

    def fail(_argv: list[str], **_kwargs: object) -> FakeResult:
        message = "cargo: not found"
        raise OSError(message)

    monkeypatch.setattr(cargo.subprocess, "run", fail)

    target = cargo.DocTarget("x", "lib", None)

    with pytest.raises(
        RuntimeError, match=r"cannot run cargo rustdoc for x lib \(lib\)"
    ):
        cargo.CargoAdapter("cargo").measure(target, "nightly-x", tmp_path)


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(
            ReportedCoverageFileCase(
                '{"src/lib.rs": {"total": 10, "with_docs": 9}}',
                pathlib.Path("coverage-reports/absolute.json"),
                None,
                (10, 9),
            ),
            id="absolute-reported-path",
        ),
        pytest.param(
            ReportedCoverageFileCase(
                '{"src/lib.rs": {"total": 7, "with_docs": 6}}',
                pathlib.Path("target/doc/package.json"),
                pathlib.Path("target/doc/package.json"),
                (7, 6),
            ),
            id="relative-reported-path",
        ),
    ],
)
def test_measure_reads_the_reported_generated_coverage_file(
    cargo: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    case: ReportedCoverageFileCase,
) -> None:
    """Read absolute and manifest-relative Rustdoc coverage-file notices."""
    FakeCargo(
        cargo,
        rustdoc=FakeRustdocResult(
            payload=case.payload,
            output_path=case.output_path,
            reported_path=case.reported_path,
        ),
    ).install(monkeypatch)

    coverage = cargo.CargoAdapter("cargo").measure(
        cargo.DocTarget("x", "lib", None), "nightly-x", tmp_path
    )

    assert coverage == cargo.Coverage(*case.expected), (
        "coverage JSON was not read from the reported generated file"
    )


@pytest.mark.parametrize(
    ("output", "expected"),
    [
        pytest.param(
            'Generated output into "/tmp/coverage.json"',
            pathlib.Path("/tmp/coverage.json"),
            id="absolute-path",
        ),
        pytest.param(
            'progress\nGenerated output into "target/doc/package.json"',
            pathlib.Path("target/doc/package.json"),
            id="relative-path",
        ),
    ],
)
def test_coverage_output_path_accepts_reported_paths(
    cargo: types.ModuleType,
    tmp_path: pathlib.Path,
    output: str,
    expected: pathlib.Path,
) -> None:
    """Parse absolute and relative paths from Rustdoc's generated-file notice."""
    target = cargo.DocTarget("x", "lib", None)

    path = cargo.coverage_output_path(target, output, tmp_path)

    assert path == (expected if expected.is_absolute() else tmp_path / expected), (
        "failed to resolve the reported coverage path"
    )


@pytest.mark.parametrize("output", ["", "progress only", 'Generated output into "'])
def test_coverage_output_path_rejects_unrelated_output(
    cargo: types.ModuleType,
    tmp_path: pathlib.Path,
    output: str,
) -> None:
    """Reject output that does not contain a complete generated-file notice."""
    target = cargo.DocTarget("x", "lib", None)

    with pytest.raises(
        RuntimeError, match="did not report the generated coverage JSON path"
    ):
        cargo.coverage_output_path(target, output, tmp_path)


def test_measure_rejects_a_reported_file_that_does_not_exist(
    cargo: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Translate a missing reported JSON file into the controlled gate error."""
    FakeCargo(
        cargo,
        rustdoc=FakeRustdocResult(
            output_path=tmp_path / "missing" / "coverage.json",
            write_output=False,
        ),
    ).install(monkeypatch)

    with pytest.raises(RuntimeError, match="cannot read generated coverage JSON"):
        cargo.CargoAdapter("cargo").measure(
            cargo.DocTarget("x", "lib", None), "nightly-x", tmp_path
        )


def test_toolchain_override_reaches_every_cargo_call(
    script: types.ModuleType,
    cargo: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """``--toolchain`` flows into the ``+channel`` selector for every call."""
    metadata = (
        '{"packages": [{"id": "pkg:x:1.0.0", "name": "x", '
        '"targets": [{"name": "x", "kind": ["lib"]}]}], '
        '"workspace_members": ["pkg:x:1.0.0"]}'
    )
    fake = FakeCargo(cargo, metadata=metadata).install(monkeypatch)
    monkeypatch.chdir(tmp_path)

    script.main(["--toolchain", "nightly-custom", "--threshold", "0"])

    assert fake.calls, "expected at least one cargo invocation"
    assert all(argv[1] == "+nightly-custom" for argv in fake.calls), (
        f"toolchain selector not threaded through: {fake.calls!r}"
    )


def test_pinned_toolchain_reads_the_channel(
    runner: types.ModuleType,
    tmp_path: pathlib.Path,
) -> None:
    """The pinned channel is recollected from rust-toolchain.toml."""
    (tmp_path / "rust-toolchain.toml").write_text(
        '[toolchain]\nchannel = "nightly-from-pin"\n',
        encoding="utf-8",
    )

    assert runner.pinned_toolchain(tmp_path) == "nightly-from-pin"


def test_parse_threshold_rejects_invalid_values(script: types.ModuleType) -> None:
    """Thresholds outside [0, 100] and non-numbers are argument errors."""
    for value in ["101", "-1", "not-a-number"]:
        with pytest.raises(argparse.ArgumentTypeError):
            script.parse_threshold(value)


def test_label_names_libraries_and_binaries(script: types.ModuleType) -> None:
    """The breakdown labels distinguish lib from named binary targets."""
    lib = script.DocTarget("netsuke", "lib", None)
    binary = script.DocTarget("netsuke", "bin", "netsuke-bin")

    assert script.label(lib) == "netsuke lib"
    assert script.label(binary) == "netsuke bin (netsuke-bin)"


@pytest.mark.parametrize(
    ("target", "selector"),
    [
        pytest.param(
            ("netsuke", "lib", None),
            ["--lib"],
            id="library",
        ),
        pytest.param(
            ("netsuke", "bin", "netsuke-bin"),
            ["--bin", "netsuke-bin"],
            id="binary",
        ),
    ],
)
def test_rustdoc_args_for_target(
    cargo: types.ModuleType,
    target: tuple[str, str, str | None],
    selector: list[str],
) -> None:
    """Build the complete rustdoc command, selecting lib or bin by target kind."""
    doc_target = cargo.DocTarget(*target)

    args = cargo.rustdoc_args(doc_target, "nightly-x", "cargo")

    assert args == [
        "cargo",
        "+nightly-x",
        "rustdoc",
        "-p",
        "netsuke",
        *selector,
        "--",
        "-Z",
        "unstable-options",
        "--show-coverage",
        "--output-format",
        "json",
        "--document-private-items",
    ]


def test_cargo_adapter_owns_rustdoc_arguments(
    cargo: types.ModuleType,
) -> None:
    """Build the unchanged Rustdoc argv through the Cargo adapter directly."""
    assert cargo.rustdoc_args(
        cargo.DocTarget("x", "lib", None), "nightly-x", "cargo-wrapper"
    )[:5] == [
        "cargo-wrapper",
        "+nightly-x",
        "rustdoc",
        "-p",
        "x",
    ]


def test_production_adapter_uses_the_configured_cargo_executable(
    cargo: types.ModuleType,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Resolve the process executable once at the production adapter boundary."""
    monkeypatch.setenv("CARGO", "cargo-wrapper")

    assert cargo.production_adapter().executable == "cargo-wrapper"


def test_runner_delegates_to_cargo_adapter(
    runner: types.ModuleType,
    tmp_path: pathlib.Path,
) -> None:
    """Delegate metadata and target measurement while retaining aggregation."""
    target = runner.DocTarget("x", "lib", None)
    coverage = runner.Coverage(3, 2)
    calls: list[object] = []

    adapter = FakeCoverageAdapter(calls, coverage)

    totals, rows = runner.run_measurements("nightly-x", tmp_path, adapter)

    assert totals == coverage
    assert rows == [(target, coverage)]
    assert calls == [
        ("metadata", "nightly-x", tmp_path),
        (target, "nightly-x", tmp_path),
    ]


def test_main_delegates_to_runner_measurements(
    script: types.ModuleType,
    runner: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Delegate CLI measurement while retaining CLI reporting and exit policy."""
    expected = runner.Coverage(1, 1)
    monkeypatch.setattr(
        runner, "run_measurements", lambda _toolchain, _root: (expected, [])
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


def test_parse_coverage_output_aggregates_multiple_files(
    cargo: types.ModuleType,
) -> None:
    """Per-file totals and with_docs counts roll up across the payload."""
    target = cargo.DocTarget("netsuke", "lib", None)
    payload = (
        '{"src/a.rs": {"total": 10, "with_docs": 8}, '
        '"src/b.rs": {"total": 5, "with_docs": 3}}'
    )

    coverage = cargo.parse_coverage_output(target, payload)

    assert coverage.total == 15
    assert coverage.with_docs == 11


def test_parse_coverage_output_rejects_malformed_json(cargo: types.ModuleType) -> None:
    """Non-JSON output surfaces as a RuntimeError naming the coverage gate."""
    target = cargo.DocTarget("netsuke", "lib", None)

    with pytest.raises(RuntimeError, match="did not emit coverage JSON"):
        cargo.parse_coverage_output(target, "not json at all")


@pytest.mark.parametrize(
    "entry",
    [
        pytest.param('{"total": true, "with_docs": 0}', id="boolean"),
        pytest.param('{"total": 1e999, "with_docs": 0}', id="non-finite"),
        pytest.param('{"total": -1, "with_docs": 0}', id="negative"),
        pytest.param('{"total": 1.5, "with_docs": 0}', id="non-integer"),
        pytest.param('{"total": 1, "with_docs": 2}', id="inconsistent"),
    ],
)
def test_main_rejects_invalid_coverage_counts(
    script: types.ModuleType,
    cargo: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    entry: str,
) -> None:
    """Return the controlled exit for invalid Rustdoc count invariants."""
    payload = '{"src/lib.rs": ' + entry + "}"
    FakeCargo(
        cargo,
        metadata=single_library_metadata(),
        rustdoc=FakeRustdocResult(payload=payload),
    ).install(monkeypatch)
    monkeypatch.chdir(tmp_path)

    with pytest.raises(RuntimeError, match="each entry requires total and with_docs"):
        script.runner.run_measurements("nightly-x", tmp_path)

    assert script.main(["--toolchain", "nightly-x"]) == 2


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(
            CoveragePayloadFailureCase("[]", "expected an object"),
            id="non-object",
        ),
        pytest.param(
            CoveragePayloadFailureCase(
                '{"src/lib.rs": {"total": 1}}',
                "each entry requires total and with_docs",
            ),
            id="missing-with-docs",
        ),
        pytest.param(
            CoveragePayloadFailureCase(
                '{"src/lib.rs": {"with_docs": 1}}',
                "each entry requires total and with_docs",
            ),
            id="missing-total",
        ),
    ],
)
def test_main_maps_invalid_coverage_shape_to_measurement_error(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    case: CoveragePayloadFailureCase,
) -> None:
    """Return the controlled measurement exit for invalid coverage JSON shapes."""
    cargo = script.runner.doc_coverage_cargo
    FakeCargo(
        cargo,
        metadata=single_library_metadata(),
        rustdoc=FakeRustdocResult(payload=case.payload),
    ).install(monkeypatch)
    monkeypatch.chdir(tmp_path)

    with pytest.raises(RuntimeError, match=case.diagnostic):
        script.runner.run_measurements("nightly-x", tmp_path)

    assert script.main(["--toolchain", "nightly-x"]) == 2
