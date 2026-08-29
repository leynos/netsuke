"""Test documentation-coverage target selection and measurement orchestration."""

import typing as typ

import pytest

if typ.TYPE_CHECKING:
    import pathlib
    import types


def lib_target(name: str) -> dict[str, object]:
    """Return one library target as ``cargo metadata`` reports it."""
    return {"name": name, "kind": ["lib"]}


def bin_target(name: str) -> dict[str, object]:
    """Return one binary target as ``cargo metadata`` reports it."""
    return {"name": name, "kind": ["bin"]}


def metadata_for(packages: list[dict[str, object]]) -> dict[str, object]:
    """Build the ``cargo metadata`` document the runner consumes."""
    return {
        "packages": packages,
        "workspace_members": [package["id"] for package in packages],
    }


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
        return metadata_for([
            {
                "id": "pkg:x:1.0.0",
                "name": "x",
                "targets": [lib_target("x")],
            }
        ])

    def measure(
        self, target: object, toolchain: str, manifest_root: pathlib.Path
    ) -> object:
        """Record the selected target and return fixed coverage."""
        self._calls.append((target, toolchain, manifest_root))
        return self._coverage


def test_target_discovery_skips_non_doc_targets(runner: types.ModuleType) -> None:
    """Exclude build scripts, tests, examples, and benches from the surface."""
    metadata = metadata_for([
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
    ])

    targets = runner.doc_targets(metadata)

    assert [(target.kind, target.name) for target in targets] == [
        ("lib", None),
        ("bin", "netsuke-bin"),
        ("bin", "extra"),
    ], "only the library and binary targets belong to the documented surface"


def test_target_discovery_excludes_outside_workspace(runner: types.ModuleType) -> None:
    """Exclude dependency crates outside ``workspace_members`` from measurement."""
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

    assert [target.package for target in targets] == ["member"], (
        "packages outside workspace_members must not be measured"
    )


@pytest.mark.parametrize(
    "metadata",
    [
        pytest.param({"packages": []}, id="missing-workspace-members"),
        pytest.param({"workspace_members": []}, id="missing-packages"),
        pytest.param(
            {"packages": {}, "workspace_members": []}, id="packages-not-a-list"
        ),
    ],
)
def test_malformed_metadata_shape_is_a_measurement_error(
    runner: types.ModuleType,
    metadata: dict[str, object],
) -> None:
    """Reject valid JSON without workspace keys rather than leaking a KeyError."""
    with pytest.raises(RuntimeError, match="lacks the workspace"):
        runner.doc_targets(metadata)


@pytest.mark.parametrize(
    "package",
    [
        pytest.param(
            {"id": "pkg:x:0.1.0", "name": "x", "targets": [{"name": "mystery"}]},
            id="target-without-kind",
        ),
        pytest.param(
            {"id": "pkg:x:0.1.0", "name": "x"},
            id="package-without-targets",
        ),
        pytest.param(
            {"id": "pkg:x:0.1.0", "targets": [lib_target("x")]},
            id="package-without-name",
        ),
        pytest.param(
            {"id": "pkg:x:0.1.0", "name": "x", "targets": [{"kind": ["bin"]}]},
            id="binary-without-name",
        ),
    ],
)
def test_unmeasurable_records_are_skipped(
    runner: types.ModuleType,
    package: dict[str, object],
) -> None:
    """Ignore package and target records that lack the fields measurement needs."""
    metadata = {
        "packages": [package],
        "workspace_members": ["pkg:x:0.1.0"],
    }

    assert runner.doc_targets(metadata) == [], (
        "incomplete metadata records must be skipped rather than measured"
    )


def test_pinned_toolchain_reads_the_channel(
    runner: types.ModuleType,
    tmp_path: pathlib.Path,
) -> None:
    """Read the pinned channel from rust-toolchain.toml."""
    (tmp_path / "rust-toolchain.toml").write_text(
        '[toolchain]\nchannel = "nightly-from-pin"\n',
        encoding="utf-8",
    )

    assert runner.pinned_toolchain(tmp_path) == "nightly-from-pin", (
        "the pinned channel must be read from the toolchain file"
    )


def test_pinned_toolchain_reports_a_missing_file(
    runner: types.ModuleType,
    tmp_path: pathlib.Path,
) -> None:
    """Translate an unreadable toolchain file into the gate's measurement error."""
    with pytest.raises(RuntimeError, match="cannot read the pinned toolchain"):
        runner.pinned_toolchain(tmp_path)


def test_runner_delegates_to_cargo_adapter(
    runner: types.ModuleType,
    tmp_path: pathlib.Path,
) -> None:
    """Delegate target work to the adapter while retaining aggregation."""
    target = runner.DocTarget("x", "lib", None)
    coverage = runner.Coverage(3, 2)
    calls: list[object] = []

    totals, rows = runner.run_measurements(
        "nightly-x", tmp_path, FakeCoverageAdapter(calls, coverage)
    )

    assert (totals, rows) == (coverage, [(target, coverage)]), (
        "the runner must aggregate exactly the adapter's per-target results"
    )
    assert calls == [
        ("metadata", "nightly-x", tmp_path),
        (target, "nightly-x", tmp_path),
    ], "the runner must thread the toolchain and manifest root through the adapter"
