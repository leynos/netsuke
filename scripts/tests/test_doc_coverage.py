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
import importlib.util
import pathlib
import sys
import types

import pytest


SCRIPT_DIRECTORY = pathlib.Path(__file__).resolve().parents[1]


@pytest.fixture(name="script")
def script_fixture() -> types.ModuleType:
    """Import ``doc-coverage.py`` under a loadable module name."""
    spec = importlib.util.spec_from_file_location(
        "doc_coverage_module", SCRIPT_DIRECTORY / "doc-coverage.py"
    )
    assert spec is not None and spec.loader is not None
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


class FakeCargo:
    """Stand-in for ``cargo metadata`` and ``cargo rustdoc`` invocations.

    Each call records its argv for later assertion and answers the metadata
    call with ``metadata`` and every rustdoc call with ``rustdoc_output``.
    """

    def __init__(
        self,
        script: types.ModuleType,
        *,
        metadata: str = '{"packages": [], "workspace_members": []}',
        rustdoc_output: str = "{}",
        rustdoc_rc: int = 0,
    ) -> None:
        self._script = script
        self.metadata_payload = metadata
        self.rustdoc_payload = rustdoc_output
        self.rustdoc_rc = rustdoc_rc
        self.calls: list[list[str]] = []

    def install(self, monkeypatch: pytest.MonkeyPatch) -> FakeCargo:
        """Replace the script's ``subprocess.run`` with this fake."""
        monkeypatch.setattr(self._script.subprocess, "run", self.run)
        return self

    def run(self, argv: list[str], **_kwargs: object) -> FakeResult:
        """Answer one Cargo invocation from the canned payloads."""
        self.calls.append(argv)
        if "metadata" in argv:
            return FakeResult(0, self.metadata_payload)
        return FakeResult(self.rustdoc_rc, self.rustdoc_payload)


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


def test_malformed_rustdoc_output_is_a_measurement_error(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Non-JSON rustdoc output surfaces as a controlled error, not a crash."""
    metadata = (
        '{"packages": [{"id": "pkg:x:1.0.0", "name": "x", '
        '"targets": [{"name": "x", "kind": ["lib"]}]}], '
        '"workspace_members": ["pkg:x:1.0.0"]}'
    )
    FakeCargo(script, metadata=metadata, rustdoc_output="not json at all").install(
        monkeypatch
    )
    monkeypatch.chdir(tmp_path)

    with pytest.raises(RuntimeError, match="did not emit coverage JSON"):
        script.run_measurements("nightly-x", tmp_path)


def test_rustdoc_failure_aborts_the_run(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A failing cargo rustdoc surfaces its stderr, not an empty aggregate."""
    metadata = (
        '{"packages": [{"id": "pkg:x:1.0.0", "name": "x", '
        '"targets": [{"name": "x", "kind": ["lib"]}]}], '
        '"workspace_members": ["pkg:x:1.0.0"]}'
    )
    FakeCargo(script, metadata=metadata, rustdoc_rc=1).install(monkeypatch)
    monkeypatch.chdir(tmp_path)

    with pytest.raises(RuntimeError, match="cargo rustdoc failed for x"):
        script.run_measurements("nightly-x", tmp_path)


def test_cargo_metadata_failure_aborts_the_run(
    script: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A failing cargo metadata run is a measurement error, not an empty gate."""

    def fail(argv: list[str], **_kwargs: object) -> FakeResult:
        return FakeResult(1, "", "filtered diagnostics")

    monkeypatch.setattr(script.subprocess, "run", fail)
    monkeypatch.chdir(tmp_path)

    with pytest.raises(RuntimeError, match="cargo metadata failed"):
        script.load_metadata("nightly-x", tmp_path)


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

    def fail(argv: list[str], **_kwargs: object) -> FakeResult:
        raise OSError("cargo: not found")

    monkeypatch.setattr(script.subprocess, "run", fail)
    monkeypatch.chdir(tmp_path)

    code = script.main(["--toolchain", "nightly-x"])

    assert code == 2


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
