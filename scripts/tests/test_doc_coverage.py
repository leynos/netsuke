"""Substantive tests for the workspace Rustdoc doc-comment coverage gate.

``scripts/doc-coverage.py`` wraps ``cargo rustdoc --show-coverage`` and
``cargo metadata``; every test in this module replaces those two subprocess
boundaries with canned responses so the script's own logic — target
discovery, aggregation, threshold exits, malformed-output handling, and
command-failure translation — is exercised without invoking Cargo at all.

The helper script cannot be imported by its file name (``doc-coverage.py``
contains a hyphen), so a fixture loads it through ``importlib`` under a
hyphen-free module name.
"""

from __future__ import annotations

import argparse
import dataclasses
import importlib.util
import pathlib
import sys
import typing as typ

import pytest

if typ.TYPE_CHECKING:
    import types

SCRIPT_DIRECTORY = pathlib.Path(__file__).resolve().parents[1]


@pytest.fixture(name="script")
def script_fixture() -> types.ModuleType:
    """Import ``doc-coverage.py`` under a loadable module name."""
    spec = importlib.util.spec_from_file_location(
        "doc_coverage_module", SCRIPT_DIRECTORY / "doc-coverage.py"
    )
    assert spec is not None, "expected import setup to produce a module spec"
    assert spec.loader is not None, "expected module spec to provide a loader"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


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


class FakeCargo:
    """Stand-in for ``cargo metadata`` and ``cargo rustdoc`` invocations.

    Each call records its argv, answers metadata with ``metadata``, and writes
    each Rustdoc payload to the generated file path that Rustdoc reports.
    """

    def __init__(
        self,
        script: types.ModuleType,
        *,
        metadata: str = '{"packages": [], "workspace_members": []}',
        rustdoc_output: str = "{}",
        rustdoc_rc: int = 0,
        rustdoc_output_path: pathlib.Path | None = None,
        rustdoc_report_path: pathlib.Path | None = None,
        should_write_rustdoc_output: bool = True,
    ) -> None:
        self._script = script
        self.metadata_payload = metadata
        self.rustdoc_payload = rustdoc_output
        self.rustdoc_rc = rustdoc_rc
        self.rustdoc_output_path = rustdoc_output_path
        self.rustdoc_report_path = rustdoc_report_path
        self.should_write_rustdoc_output = should_write_rustdoc_output
        self.calls: list[list[str]] = []

    def install(self, monkeypatch: pytest.MonkeyPatch) -> FakeCargo:
        """Replace the script's ``subprocess.run`` with this fake."""
        monkeypatch.setattr(self._script.subprocess, "run", self.run)
        return self

    def run(self, argv: list[str], **kwargs: object) -> FakeResult:
        """Answer one Cargo invocation from the canned payloads."""
        self.calls.append(argv)
        if "metadata" in argv:
            return FakeResult(0, self.metadata_payload)
        manifest_root = pathlib.Path(typ.cast("pathlib.Path", kwargs["cwd"]))
        package = argv[argv.index("-p") + 1].replace("-", "_")
        configured_output_path = self.rustdoc_output_path or (
            manifest_root / "target" / "doc" / f"{package}.json"
        )
        output_path = (
            configured_output_path
            if configured_output_path.is_absolute()
            else manifest_root / configured_output_path
        )
        if self.should_write_rustdoc_output:
            output_path.parent.mkdir(parents=True, exist_ok=True)
            output_path.write_text(self.rustdoc_payload, encoding="utf-8")
        reported_path = self.rustdoc_report_path or output_path
        return FakeResult(
            self.rustdoc_rc,
            f'Generated output into "{reported_path}"\n',
        )


class FakeResult:
    """Minimal ``subprocess.CompletedProcess`` stand-in."""

    def __init__(self, returncode: int, stdout: str, stderr: str = "") -> None:
        self.returncode = returncode
        self.stdout = stdout
        self.stderr = stderr


def test_target_discovery_skips_non_doc_targets(script: types.ModuleType) -> None:
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

    targets = script.doc_targets(metadata)

    assert [target.kind for target in targets] == ["lib", "bin", "bin"]
    assert {target.name for target in targets if target.kind == "bin"} == {
        "netsuke-bin",
        "extra",
    }


def test_target_discovery_excludes_outside_workspace(script: types.ModuleType) -> None:
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

    targets = script.doc_targets(metadata)

    assert [target.package for target in targets] == ["member"]


def test_aggregation_sums_targets_and_reports_percentage(
    script: types.ModuleType,
) -> None:
    """Aggregate totals roll per-target counts up and report the share."""
    first = script.Coverage(10, 8)
    second = script.Coverage(5, 5)

    combined = first + second

    assert combined.total == 15
    assert combined.with_docs == 13
    assert combined.percentage == pytest.approx(13 / 15 * 100)


def test_empty_run_is_complete_not_a_division_by_zero(script: types.ModuleType) -> None:
    """A crate with no doc-able targets contributes an empty, complete run."""
    assert script.Coverage(0, 0).percentage == 100.0


def test_threshold_flips_exit_code(
    script: types.ModuleType,
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
    FakeCargo(script, metadata=metadata, rustdoc_output=rustdoc).install(monkeypatch)
    monkeypatch.chdir(tmp_path)

    passing = script.main(["--toolchain", "nightly-x", "--threshold", "50"])
    failing = script.main(["--toolchain", "nightly-x", "--threshold", "80"])

    assert passing == 0
    assert failing == 1


def test_cargo_metadata_failure_aborts_the_run(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A failing cargo metadata run is a measurement error, not an empty gate."""

    def fail(_argv: list[str], **_kwargs: object) -> FakeResult:
        return FakeResult(1, "", "filtered diagnostics")

    monkeypatch.setattr(script.subprocess, "run", fail)
    monkeypatch.chdir(tmp_path)

    with pytest.raises(RuntimeError, match="cargo metadata failed"):
        script.load_metadata("nightly-x", tmp_path)


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
def test_run_measurements_propagates_rustdoc_failure(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    case: RustdocFailureCase,
) -> None:
    """Propagate malformed output and non-zero rustdoc exits as measurement errors."""
    FakeCargo(
        script,
        metadata=single_library_metadata(),
        rustdoc_output=case.output,
        rustdoc_rc=case.returncode,
    ).install(monkeypatch)
    monkeypatch.chdir(tmp_path)

    with pytest.raises(RuntimeError, match=case.diagnostic):
        script.run_measurements("nightly-x", tmp_path)


def test_malformed_metadata_shape_is_a_measurement_error(
    script: types.ModuleType,
) -> None:
    """Valid JSON without workspace keys is rejected, not a KeyError crash."""
    with pytest.raises(RuntimeError, match="lacks the workspace"):
        script.doc_targets({"packages": []})


def test_target_without_kind_is_skipped(script: types.ModuleType) -> None:
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

    assert script.doc_targets(metadata) == []


def test_missing_cargo_maps_to_measurement_error(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """An OSError from Cargo exits 2 with a message, not a traceback."""

    def fail(_argv: list[str], **_kwargs: object) -> FakeResult:
        message = "cargo: not found"
        raise OSError(message)

    monkeypatch.setattr(script.subprocess, "run", fail)
    monkeypatch.chdir(tmp_path)

    code = script.main(["--toolchain", "nightly-x"])

    assert code == 2


def test_measure_maps_missing_cargo_to_measurement_error(
    script: types.ModuleType,
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

    monkeypatch.setattr(script.subprocess, "run", fail)

    target = script.DocTarget("x", "lib", None)

    with pytest.raises(
        RuntimeError, match=r"cannot run cargo rustdoc for x lib \(lib\)"
    ):
        script.measure(target, "nightly-x", tmp_path)


def test_measure_reads_coverage_from_the_reported_generated_file(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Rustdoc's output notice points the collector at the JSON payload."""
    output_path = tmp_path / "coverage-reports" / "absolute.json"
    FakeCargo(
        script,
        rustdoc_output='{"src/lib.rs": {"total": 10, "with_docs": 9}}',
        rustdoc_output_path=output_path,
    ).install(monkeypatch)

    coverage = script.measure(script.DocTarget("x", "lib", None), "nightly-x", tmp_path)

    assert coverage == script.Coverage(total=10, with_docs=9), (
        "coverage JSON was not read from the reported file"
    )


def test_measure_resolves_a_relative_reported_coverage_path(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Resolve Rustdoc's relative notice while reading the exact written file."""
    relative_path = pathlib.Path("target/doc/package.json")
    FakeCargo(
        script,
        rustdoc_output='{"src/lib.rs": {"total": 7, "with_docs": 6}}',
        rustdoc_output_path=relative_path,
        rustdoc_report_path=relative_path,
    ).install(monkeypatch)

    coverage = script.measure(script.DocTarget("x", "lib", None), "nightly-x", tmp_path)

    assert coverage == script.Coverage(total=7, with_docs=6), (
        "coverage JSON was not read from the reported relative file"
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
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    output: str,
    expected: pathlib.Path,
) -> None:
    """Parse absolute and relative paths from Rustdoc's generated-file notice."""
    target = script.DocTarget("x", "lib", None)

    path = script.coverage_output_path(target, output, tmp_path)

    assert path == (expected if expected.is_absolute() else tmp_path / expected)


@pytest.mark.parametrize("output", ["", "progress only", 'Generated output into "'])
def test_coverage_output_path_rejects_unrelated_output(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    output: str,
) -> None:
    """Reject output that does not contain a complete generated-file notice."""
    target = script.DocTarget("x", "lib", None)

    with pytest.raises(RuntimeError, match="did not report the generated coverage JSON path"):
        script.coverage_output_path(target, output, tmp_path)


def test_measure_rejects_a_reported_file_that_does_not_exist(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Translate a missing reported JSON file into the controlled gate error."""
    FakeCargo(
        script,
        rustdoc_output_path=tmp_path / "missing" / "coverage.json",
        should_write_rustdoc_output=False,
    ).install(monkeypatch)

    with pytest.raises(RuntimeError, match="cannot read generated coverage JSON"):
        script.measure(script.DocTarget("x", "lib", None), "nightly-x", tmp_path)


def test_toolchain_override_reaches_every_cargo_call(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """``--toolchain`` flows into the ``+channel`` selector for every call."""
    metadata = (
        '{"packages": [{"id": "pkg:x:1.0.0", "name": "x", '
        '"targets": [{"name": "x", "kind": ["lib"]}]}], '
        '"workspace_members": ["pkg:x:1.0.0"]}'
    )
    fake = FakeCargo(script, metadata=metadata).install(monkeypatch)
    monkeypatch.chdir(tmp_path)

    script.main(["--toolchain", "nightly-custom", "--threshold", "0"])

    assert fake.calls, "expected at least one cargo invocation"
    assert all(argv[1] == "+nightly-custom" for argv in fake.calls), (
        f"toolchain selector not threaded through: {fake.calls!r}"
    )


def test_pinned_toolchain_reads_the_channel(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
) -> None:
    """The pinned channel is recollected from rust-toolchain.toml."""
    (tmp_path / "rust-toolchain.toml").write_text(
        '[toolchain]\nchannel = "nightly-from-pin"\n',
        encoding="utf-8",
    )

    assert script.pinned_toolchain(tmp_path) == "nightly-from-pin"


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
    script: types.ModuleType,
    monkeypatch: pytest.MonkeyPatch,
    target: tuple[str, str, str | None],
    selector: list[str],
) -> None:
    """Build the complete rustdoc command, selecting lib or bin by target kind."""
    monkeypatch.setenv("CARGO", "cargo")
    doc_target = script.DocTarget(*target)

    args = script.rustdoc_args(doc_target, "nightly-x")

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


def test_parse_coverage_output_aggregates_multiple_files(
    script: types.ModuleType,
) -> None:
    """Per-file totals and with_docs counts roll up across the payload."""
    target = script.DocTarget("netsuke", "lib", None)
    payload = (
        '{"src/a.rs": {"total": 10, "with_docs": 8}, '
        '"src/b.rs": {"total": 5, "with_docs": 3}}'
    )

    coverage = script.parse_coverage_output(target, payload)

    assert coverage.total == 15
    assert coverage.with_docs == 11


def test_parse_coverage_output_rejects_malformed_json(script: types.ModuleType) -> None:
    """Non-JSON output surfaces as a RuntimeError naming the coverage gate."""
    target = script.DocTarget("netsuke", "lib", None)

    with pytest.raises(RuntimeError, match="did not emit coverage JSON"):
        script.parse_coverage_output(target, "not json at all")


@pytest.mark.parametrize(
    "entry",
    [
        pytest.param('{"total": 1e999, "with_docs": 0}', id="non-finite"),
        pytest.param('{"total": -1, "with_docs": 0}', id="negative"),
        pytest.param('{"total": 1.5, "with_docs": 0}', id="non-integer"),
        pytest.param('{"total": 1, "with_docs": 2}', id="inconsistent"),
    ],
)
def test_main_rejects_invalid_coverage_counts(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    entry: str,
) -> None:
    """Return the controlled exit for invalid Rustdoc count invariants."""
    payload = '{"src/lib.rs": ' + entry + "}"
    FakeCargo(
        script,
        metadata=single_library_metadata(),
        rustdoc_output=payload,
    ).install(monkeypatch)
    monkeypatch.chdir(tmp_path)

    with pytest.raises(RuntimeError, match="each entry requires total and with_docs"):
        script.run_measurements("nightly-x", tmp_path)

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
    FakeCargo(
        script,
        metadata=single_library_metadata(),
        rustdoc_output=case.payload,
    ).install(monkeypatch)
    monkeypatch.chdir(tmp_path)

    with pytest.raises(RuntimeError, match=case.diagnostic):
        script.run_measurements("nightly-x", tmp_path)

    assert script.main(["--toolchain", "nightly-x"]) == 2
