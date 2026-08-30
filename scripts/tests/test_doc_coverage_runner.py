"""Test documentation-coverage target selection and measurement orchestration."""

import typing as typ

import doc_coverage_runner as runner_module
import pytest
from hypothesis import given
from hypothesis import strategies as st

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


@given(
    package_name=st.one_of(st.text(), st.integers(), st.none()),
    target_name=st.one_of(st.text(), st.integers(), st.none()),
    target_kinds=st.lists(st.one_of(st.text(), st.integers(), st.none())),
)
def test_target_discovery_accepts_only_documentable_package_records(
    package_name: object,
    target_name: object,
    target_kinds: list[object],
) -> None:
    """Measure only valid library or binary records from Cargo metadata."""
    package = {
        "id": "pkg:generated:0.1.0",
        "name": package_name,
        "targets": [{"name": target_name, "kind": target_kinds}],
    }

    targets = runner_module.doc_targets({
        "packages": [package],
        "workspace_members": [package["id"]],
    })

    expected: list[object] = []
    match package_name, target_name, target_kinds:
        case str() as name, _, kinds if "lib" in kinds:
            expected = [runner_module.DocTarget(name, "lib", None)]
        case str() as name, str() as binary, kinds if "bin" in kinds:
            expected = [runner_module.DocTarget(name, "bin", binary)]
    assert targets == expected, (
        "only complete library and binary metadata records are measurable"
    )


@pytest.mark.parametrize(
    "metadata",
    [
        pytest.param({"packages": []}, id="missing-workspace-members"),
        pytest.param({"workspace_members": []}, id="missing-packages"),
        pytest.param(
            {"packages": {}, "workspace_members": []}, id="packages-not-a-list"
        ),
        pytest.param(
            {"packages": [], "workspace_members": [[]]},
            id="workspace-member-not-a-string",
        ),
    ],
)
def test_malformed_metadata_shape_is_a_measurement_error(
    runner: types.ModuleType,
    metadata: dict[str, object],
) -> None:
    """Reject valid JSON without workspace keys rather than leaking a KeyError."""
    with pytest.raises(runner.WorkspaceMetadataError, match="lacks the workspace"):
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
    with pytest.raises(
        runner.ToolchainPinError, match="cannot read the pinned toolchain"
    ):
        runner.pinned_toolchain(tmp_path)


@pytest.mark.parametrize(
    ("contents", "scenario"),
    [
        pytest.param('toolchain = "nightly"\n', "scalar-toolchain", id="scalar"),
        pytest.param(
            "[toolchain]\nchannel = 314\n", "non-string-channel", id="integer"
        ),
        pytest.param('[toolchain]\nchannel = ""\n', "empty-channel", id="empty"),
    ],
)
def test_pinned_toolchain_rejects_malformed_records(
    runner: types.ModuleType,
    tmp_path: pathlib.Path,
    contents: str,
    scenario: str,
) -> None:
    """Reject scalar toolchains and unusable channels at the pin boundary."""
    (tmp_path / "rust-toolchain.toml").write_text(contents, encoding="utf-8")

    with pytest.raises(runner.ToolchainPinError):
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
