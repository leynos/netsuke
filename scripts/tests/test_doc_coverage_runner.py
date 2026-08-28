"""Test documentation-coverage target selection and measurement orchestration."""

from __future__ import annotations

import pathlib
import types

import pytest


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


def test_target_discovery_skips_non_doc_targets(runner: types.ModuleType) -> None:
    """Exclude build scripts, tests, examples, and benches from the surface."""
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

    assert [target.package for target in targets] == ["member"]


def test_malformed_metadata_shape_is_a_measurement_error(
    runner: types.ModuleType,
) -> None:
    """Reject valid JSON without workspace keys rather than leaking a KeyError."""
    with pytest.raises(RuntimeError, match="lacks the workspace"):
        runner.doc_targets({"packages": []})


def test_target_without_kind_is_skipped(runner: types.ModuleType) -> None:
    """Ignore a target record that lacks its kind list."""
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


def test_pinned_toolchain_reads_the_channel(
    runner: types.ModuleType,
    tmp_path: pathlib.Path,
) -> None:
    """Read the pinned channel from rust-toolchain.toml."""
    (tmp_path / "rust-toolchain.toml").write_text(
        '[toolchain]\nchannel = "nightly-from-pin"\n',
        encoding="utf-8",
    )

    assert runner.pinned_toolchain(tmp_path) == "nightly-from-pin"


def test_runner_delegates_to_cargo_adapter(
    runner: types.ModuleType,
    tmp_path: pathlib.Path,
) -> None:
    """Delegate target work to the adapter while retaining aggregation."""
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
