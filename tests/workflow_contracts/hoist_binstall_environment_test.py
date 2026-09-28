"""Verify the release hoist reads its version from the process environment."""

import typing as typ

if typ.TYPE_CHECKING:
    import pathlib

    import pytest

import hoist_binstall_archives as hoist_mod
from conftest import EXPECTED_NAMES, VERSION, stage_pair


def test_main_reads_the_release_version_from_input_environment(
    workspace: dict[str, pathlib.Path], monkeypatch: pytest.MonkeyPatch
) -> None:
    """The reusable workflow can supply the version through INPUT_VERSION."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64/s2", EXPECTED_NAMES[0])
    monkeypatch.setenv("INPUT_VERSION", VERSION)

    status = hoist_mod.main([
        "--dist-dir",
        str(workspace["dist"]),
        "--staging-config",
        str(workspace["staging"]),
        "--manifest",
        str(workspace["manifest"]),
    ])

    assert status == 0, "the INPUT_VERSION invocation must hoist staged assets"
    for name in EXPECTED_NAMES:
        assert (workspace["dist"] / name).is_file(), (
            f"the environment-driven invocation must move {name} to the dist root"
        )
