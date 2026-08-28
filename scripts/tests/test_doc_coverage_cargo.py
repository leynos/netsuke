"""Test the Cargo and Rustdoc documentation-coverage adapter."""

from __future__ import annotations

import dataclasses
import pathlib
import types
import typing as typ

import pytest


def single_library_metadata() -> str:
    """Return metadata JSON for one package with one library target."""
    return (
        '{"packages": [{"id": "pkg:x:1.0.0", "name": "x", '
        '"targets": [{"name": "x", "kind": ["lib"]}]}], '
        '"workspace_members": ["pkg:x:1.0.0"]}'
    )


@dataclasses.dataclass(frozen=True)
class RustdocFailureCase:
    """Define one Rustdoc failure scenario for measurement integration tests."""

    output: str
    returncode: int
    diagnostic: str


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


class FakeResult:
    """Provide a minimal ``subprocess.CompletedProcess`` stand-in."""

    def __init__(self, returncode: int, stdout: str, stderr: str = "") -> None:
        self.returncode = returncode
        self.stdout = stdout
        self.stderr = stderr


class FakeCargo:
    """Stand in for ``cargo metadata`` and ``cargo rustdoc`` invocations.

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


def test_cargo_metadata_failure_aborts_the_run(
    cargo: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Treat a failing cargo metadata run as a measurement error."""

    def fail(_argv: list[str], **_kwargs: object) -> FakeResult:
        """Return the configured metadata failure."""
        return FakeResult(1, "", "filtered diagnostics")

    monkeypatch.setattr(cargo.subprocess, "run", fail)
    monkeypatch.chdir(tmp_path)

    with pytest.raises(RuntimeError, match="cargo metadata failed"):
        cargo.CargoAdapter("cargo").load_metadata("nightly-x", tmp_path)


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(
            RustdocFailureCase("not json at all", 0, "did not emit coverage JSON"),
            id="malformed-output",
        ),
        pytest.param(
            RustdocFailureCase("{}", 1, "cargo rustdoc failed for x"),
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
    """Translate malformed output and process failures into measurement errors."""
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


def test_measure_maps_missing_cargo_to_measurement_error(
    cargo: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Translate an OSError from Rustdoc execution into a RuntimeError."""

    def fail(_argv: list[str], **_kwargs: object) -> FakeResult:
        """Raise the configured Cargo executable error."""
        raise OSError("cargo: not found")

    monkeypatch.setattr(cargo.subprocess, "run", fail)

    with pytest.raises(
        RuntimeError, match=r"cannot run cargo rustdoc for x lib \(lib\)"
    ):
        cargo.CargoAdapter("cargo").measure(
            cargo.DocTarget("x", "lib", None), "nightly-x", tmp_path
        )


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


@pytest.mark.parametrize(
    ("target", "selector"),
    [
        pytest.param(("netsuke", "lib", None), ["--lib"], id="library"),
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
    """Build the complete Rustdoc command for library and binary targets."""
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


def test_cargo_adapter_owns_rustdoc_arguments(cargo: types.ModuleType) -> None:
    """Build the unchanged Rustdoc argv through the Cargo adapter directly."""
    assert cargo.rustdoc_args(
        cargo.DocTarget("x", "lib", None), "nightly-x", "cargo-wrapper"
    )[:5] == ["cargo-wrapper", "+nightly-x", "rustdoc", "-p", "x"]


def test_production_adapter_uses_the_configured_cargo_executable(
    cargo: types.ModuleType,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Resolve the process executable once at the production adapter boundary."""
    monkeypatch.setenv("CARGO", "cargo-wrapper")

    assert cargo.production_adapter().executable == "cargo-wrapper"
