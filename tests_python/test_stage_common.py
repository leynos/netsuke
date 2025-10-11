"""Tests for the reusable staging helper module.

Summary
-------
Validate that the TOML loader merges configuration blocks correctly and that
``stage_artefacts`` stages binaries, manuals, and licences while publishing
useful metadata for later workflow steps.

Usage
-----
Run the tests directly::

    uvx --with "cyclopts>=0.14" pytest tests_python/test_stage_common.py
"""

from __future__ import annotations

import importlib
import json
import os
import sys
from pathlib import Path, PurePosixPath, PureWindowsPath

import pytest
from stage_test_helpers import decode_output_file, write_workspace_inputs

REPO_ROOT = Path(__file__).resolve().parent.parent
MODULE_DIR = REPO_ROOT / ".github" / "actions" / "stage" / "scripts"


@pytest.fixture(scope="session")
def stage_common() -> object:
    """Load the staging helper package once for reuse across tests."""
    sys_path = str(MODULE_DIR)

    sys.path.insert(0, sys_path)
    try:
        return importlib.import_module("stage_common")
    finally:
        sys.path.remove(sys_path)


@pytest.fixture
def staging_module(stage_common: object) -> object:
    """Expose the staging implementation module for unit-level assertions."""

    return importlib.import_module("stage_common.staging")


@pytest.fixture
def workspace(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Create an isolated workspace and set ``GITHUB_WORKSPACE`` accordingly."""
    root = tmp_path / "workspace"
    root.mkdir()
    monkeypatch.setenv("GITHUB_WORKSPACE", str(root))
    return root


def test_public_interface(stage_common: object) -> None:
    """The package should expose the documented public API."""
    expected = {
        "ArtefactConfig",
        "StageError",
        "StageResult",
        "StagingConfig",
        "load_config",
        "require_env_path",
        "stage_artefacts",
    }
    assert set(stage_common.__all__) == expected


def test_stage_error_is_runtime_error(stage_common: object) -> None:
    """``StageError`` should subclass :class:`RuntimeError`."""
    error = stage_common.StageError("boom")
    assert isinstance(error, RuntimeError)
    assert str(error) == "boom"


def test_initialize_staging_dir_removes_existing_contents(
    staging_module: object, tmp_path: Path
) -> None:
    """The helper should clear any previous staging directory contents."""

    staging_dir = tmp_path / "stage"
    stale_file = staging_dir / "stale.txt"
    stale_file.parent.mkdir(parents=True, exist_ok=True)
    stale_file.write_text("old", encoding="utf-8")

    staging_module._initialize_staging_dir(staging_dir)

    assert staging_dir.exists(), "Expected staging directory to be recreated"
    assert list(staging_dir.iterdir()) == [], "Stale artefacts should be removed"


def test_prepare_output_data_returns_sorted_metadata(
    staging_module: object, tmp_path: Path
) -> None:
    """Output preparation should normalise ordering and serialise metadata."""

    staging_dir = tmp_path / "dist" / "stage"
    staged = [
        staging_dir / "b.bin",
        staging_dir / "a.txt",
    ]
    outputs = {
        "binary": staging_dir / "b.bin",
        "manual": staging_dir / "a.txt",
    }
    checksums = {"b.bin": "bbb", "a.txt": "aaa"}

    result = staging_module._prepare_output_data(
        staging_dir, staged, outputs, checksums
    )

    assert result["artifact_dir"].endswith("stage"), "Expected staging directory output"
    assert result["dist_dir"].endswith("dist"), "Expected dist directory output"
    assert result["staged_files"].splitlines() == [
        "a.txt",
        "b.bin",
    ], "Staged files should be sorted"
    artefact_map = json.loads(result["artefact_map"])
    assert list(artefact_map) == ["binary", "manual"], "Outputs should be sorted"
    checksum_map = json.loads(result["checksum_map"])
    assert list(checksum_map) == ["a.txt", "b.bin"], "Checksums should be sorted"


def test_validate_no_reserved_key_collisions_rejects_reserved_keys(
    staging_module: object
) -> None:
    """Reserved workflow keys should trigger a stage error."""

    with pytest.raises(staging_module.StageError) as exc:
        staging_module._validate_no_reserved_key_collisions(
            {"artifact_dir": Path("/tmp/stage")}
        )

    assert "collide with reserved keys" in str(exc.value)


def test_write_github_output_formats_values(
    staging_module: object, tmp_path: Path
) -> None:
    """The GitHub output helper should escape strings and stream lists."""

    output_file = tmp_path / "github" / "output.txt"
    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text("initial=value\n", encoding="utf-8")

    staging_module.write_github_output(
        output_file,
        {
            "name": "value with%percent\nand newline",
            "lines": ["one", "two"],
        },
    )

    content = output_file.read_text(encoding="utf-8")
    assert "initial=value" in content, "Existing output lines should remain"
    assert (
        "name=value with%25percent%0Aand newline" in content
    ), "String values should be escaped"
    assert (
        "lines<<gh_LINES" in content
    ), "List values should use the multi-line protocol"
    assert "one\ntwo" in content, "List payload should be preserved"


def test_require_env_path_returns_path(stage_common: object, workspace: Path) -> None:
    """The environment helper should return a ``Path`` when set."""
    path = stage_common.require_env_path("GITHUB_WORKSPACE")
    assert path == workspace


def test_require_env_path_missing_env(
    stage_common: object, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A missing environment variable should raise ``StageError``."""
    monkeypatch.delenv("GITHUB_WORKSPACE", raising=False)
    with pytest.raises(stage_common.StageError) as exc:
        stage_common.require_env_path("GITHUB_WORKSPACE")
    assert "Environment variable 'GITHUB_WORKSPACE' is not set" in str(exc.value)


def test_staging_config_template_context(stage_common: object, workspace: Path) -> None:
    """The configuration should expose a rich template context."""
    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[],
        platform="linux",
        arch="amd64",
        target="x86_64-unknown-linux-gnu",
        bin_ext=".exe",
        target_key="linux-x86_64",
    )

    context = config.as_template_context()

    assert context["workspace"] == workspace.as_posix()
    assert context["staging_dir_name"] == "netsuke_linux_amd64"
    assert context["staging_dir_template"] == "{bin_name}_{platform}_{arch}"
    assert context["target_key"] == "linux-x86_64"


def test_load_config_merges_common_and_target(
    stage_common: object, workspace: Path
) -> None:
    """``load_config`` should merge common values with the requested target."""
    config_file = workspace / "release-staging.toml"
    config_file.write_text(
        """\
[common]
bin_name = "netsuke"
dist_dir = "dist"
checksum_algorithm = "sha256"
artefacts = [
  { source = "target/{target}/release/{bin_name}{bin_ext}", required = true, output = "binary_path" },
  { source = "LICENSE", required = true, output = "license_path" },
]

[targets.test]
platform = "linux"
arch = "amd64"
target = "x86_64-unknown-linux-gnu"
""",
        encoding="utf-8",
    )

    config = stage_common.load_config(config_file, "test")

    assert config.workspace == workspace
    assert config.bin_name == "netsuke"
    assert config.platform == "linux"
    assert config.arch == "amd64"
    assert config.target == "x86_64-unknown-linux-gnu"
    assert config.checksum_algorithm == "sha256"
    assert [item.output for item in config.artefacts] == ["binary_path", "license_path"]


def test_load_config_reads_repository_file(
    stage_common: object, workspace: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The repository TOML configuration should parse without modification."""
    config_source = REPO_ROOT / ".github" / "release-staging.toml"
    config_copy = workspace / "release-staging.toml"
    config_copy.write_text(config_source.read_text(encoding="utf-8"), encoding="utf-8")

    monkeypatch.setenv("GITHUB_WORKSPACE", str(workspace))

    config = stage_common.load_config(config_copy, "linux-x86_64")

    assert config.bin_name == "netsuke"
    assert config.staging_dir().name == "netsuke_linux_amd64"
    assert {item.output for item in config.artefacts} >= {
        "binary_path",
        "man_path",
        "license_path",
    }


def test_load_config_requires_workspace_env(
    stage_common: object, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """``load_config`` should fail when ``GITHUB_WORKSPACE`` is unset."""
    config_file = tmp_path / "release-staging.toml"
    config_file.write_text(
        """\
[common]
bin_name = "netsuke"
checksum_algorithm = "sha256"
artefacts = [ { source = "LICENSE" } ]

[targets.test]
platform = "linux"
arch = "amd64"
target = "x86_64-unknown-linux-gnu"
""",
        encoding="utf-8",
    )

    monkeypatch.delenv("GITHUB_WORKSPACE", raising=False)

    with pytest.raises(stage_common.StageError) as exc:
        stage_common.load_config(config_file, "test")
    assert "Environment variable 'GITHUB_WORKSPACE' is not set" in str(exc.value)


def test_load_config_rejects_unknown_checksum(
    stage_common: object, workspace: Path
) -> None:
    """Unsupported checksum algorithms should raise ``StageError``."""
    config_file = workspace / "release-staging.toml"
    config_file.write_text(
        """\
[common]
bin_name = "netsuke"
checksum_algorithm = "unknown"
artefacts = [ { source = "LICENSE" } ]

[targets.test]
platform = "linux"
arch = "amd64"
target = "x86_64-unknown-linux-gnu"
""",
        encoding="utf-8",
    )

    with pytest.raises(stage_common.StageError) as exc:
        stage_common.load_config(config_file, "test")
    assert "Unsupported checksum algorithm" in str(exc.value)


def test_load_config_requires_common_bin_name(
    stage_common: object, workspace: Path
) -> None:
    """Missing ``bin_name`` in ``[common]`` should raise ``StageError``."""
    config_file = workspace / "release-staging.toml"
    config_file.write_text(
        """\
[common]
checksum_algorithm = "sha256"
artefacts = [ { source = "LICENSE" } ]

[targets.test]
arch = "amd64"
target = "x86_64-unknown-linux-gnu"
platform = "linux"
""",
        encoding="utf-8",
    )

    with pytest.raises(stage_common.StageError) as exc:
        stage_common.load_config(config_file, "test")

    message = str(exc.value)
    assert "bin_name" in message
    assert "[common]" in message


def test_load_config_requires_target_platform(
    stage_common: object, workspace: Path
) -> None:
    """Missing target metadata should raise ``StageError`` with guidance."""
    config_file = workspace / "release-staging.toml"
    config_file.write_text(
        """\
[common]
bin_name = "netsuke"
checksum_algorithm = "sha256"
artefacts = [ { source = "LICENSE" } ]

[targets.test]
arch = "amd64"
target = "x86_64-unknown-linux-gnu"
""",
        encoding="utf-8",
    )

    with pytest.raises(stage_common.StageError) as exc:
        stage_common.load_config(config_file, "test")

    message = str(exc.value)
    assert "platform" in message
    assert "[targets.test]" in message


def test_load_config_requires_artefact_source(
    stage_common: object, workspace: Path
) -> None:
    """Artefact entries must define ``source`` for friendly errors."""
    config_file = workspace / "release-staging.toml"
    config_file.write_text(
        """\
[common]
bin_name = "netsuke"
checksum_algorithm = "sha256"
artefacts = [ { output = "binary" } ]

[targets.test]
platform = "linux"
arch = "amd64"
target = "x86_64-unknown-linux-gnu"
""",
        encoding="utf-8",
    )

    with pytest.raises(stage_common.StageError) as exc:
        stage_common.load_config(config_file, "test")

    message = str(exc.value)
    assert "source" in message
    assert "entry #1" in message


def test_load_config_requires_target_section(
    stage_common: object, workspace: Path
) -> None:
    """Missing target sections should raise ``StageError``."""
    config_file = workspace / "release-staging.toml"
    config_file.write_text(
        """\
[common]
bin_name = "netsuke"
checksum_algorithm = "sha256"
artefacts = [ { source = "LICENSE" } ]

[targets.other]
platform = "linux"
arch = "amd64"
target = "x86_64-unknown-linux-gnu"
""",
        encoding="utf-8",
    )

    with pytest.raises(stage_common.StageError) as exc:
        stage_common.load_config(config_file, "test")
    assert "Missing configuration key" in str(exc.value)


def test_stage_artefacts_exports_metadata(
    stage_common: object, workspace: Path
) -> None:
    """The staging pipeline should copy inputs, hash them, and export outputs."""
    target = "x86_64-unknown-linux-gnu"
    write_workspace_inputs(workspace, target)

    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="target/{target}/release/{bin_name}{bin_ext}",
                required=True,
                output="binary_path",
            ),
            stage_common.ArtefactConfig(
                source="target/generated-man/{target}/release/{bin_name}.1",
                required=True,
                output="man_path",
            ),
            stage_common.ArtefactConfig(
                source="LICENSE",
                required=True,
                output="license_path",
            ),
        ],
        platform="linux",
        arch="amd64",
        target=target,
    )

    github_output = workspace / "outputs.txt"
    result = stage_common.stage_artefacts(config, github_output)

    staging_dir = workspace / "dist" / "netsuke_linux_amd64"
    assert result.staging_dir == staging_dir, "StageResult must record the staging directory"
    assert staging_dir.exists(), "Expected staging directory to be created"

    staged_files = {path.name for path in result.staged_artefacts}
    assert staged_files == {"netsuke", "netsuke.1", "LICENSE"}, "Unexpected artefacts staged"
    assert set(result.outputs) == {"binary_path", "man_path", "license_path"}, "Outputs missing expected keys"
    expected_checksums = {
        "netsuke": staging_dir / "netsuke.sha256",
        "netsuke.1": staging_dir / "netsuke.1.sha256",
        "LICENSE": staging_dir / "LICENSE.sha256",
    }
    assert set(result.checksums) == set(expected_checksums), "Checksum outputs missing entries"
    for path in expected_checksums.values():
        assert path.exists(), f"Checksum file {path.name} was not written"

    outputs = decode_output_file(github_output)
    assert outputs["artifact_dir"] == staging_dir.as_posix(), "artifact_dir output should reference staging directory"
    assert outputs["binary_path"].endswith("netsuke"), "binary_path output should point to the staged executable"
    assert outputs["license_path"].endswith("LICENSE"), "license_path output should point to the staged licence"
    assert outputs["dist_dir"].endswith("dist"), "dist_dir output should reflect parent directory"
    staged_listing = outputs["staged_files"].splitlines()
    assert staged_listing == sorted(staged_listing), "Staged files output should be sorted"
    artefact_map = json.loads(outputs["artefact_map"])
    assert artefact_map["binary_path"].endswith("netsuke"), "artefact map should include the binary path"
    assert artefact_map["license_path"].endswith("LICENSE"), "artefact map should include the licence path"
    checksum_map = json.loads(outputs["checksum_map"])
    assert set(checksum_map) == {"netsuke", "netsuke.1", "LICENSE"}, "Checksum map missing entries"


def test_stage_artefacts_reinitialises_staging_dir(
    stage_common: object, workspace: Path
) -> None:
    """Running the pipeline should recreate the staging directory afresh."""

    target = "x86_64-unknown-linux-gnu"
    write_workspace_inputs(workspace, target)

    staging_dir = workspace / "dist" / "netsuke_linux_amd64"
    stale = staging_dir / "obsolete.txt"
    stale.parent.mkdir(parents=True, exist_ok=True)
    stale.write_text("stale", encoding="utf-8")

    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="target/{target}/release/{bin_name}{bin_ext}",
                required=True,
                output="binary_path",
            ),
        ],
        platform="linux",
        arch="amd64",
        target=target,
    )

    github_output = workspace / "outputs.txt"
    stage_common.stage_artefacts(config, github_output)

    assert not stale.exists(), "Previous staging artefacts should be removed"
    current_entries = {path.name for path in staging_dir.iterdir()}
    assert "obsolete.txt" not in current_entries, "Old entries must not survive reinitialisation"


def test_stage_artefacts_rejects_reserved_output_key(
    stage_common: object, workspace: Path
) -> None:
    """Configs using reserved workflow outputs should error out."""

    target = "x86_64-unknown-linux-gnu"
    write_workspace_inputs(workspace, target)

    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="LICENSE",
                required=True,
                output="artifact_dir",
            ),
        ],
        platform="linux",
        arch="amd64",
        target=target,
    )

    github_output = workspace / "outputs.txt"
    github_output.write_text("", encoding="utf-8")
    with pytest.raises(stage_common.StageError) as exc:
        stage_common.stage_artefacts(config, github_output)

    assert "collide with reserved keys" in str(exc.value)
    assert github_output.read_text(encoding="utf-8") == "", "Outputs should not be written when validation fails"


def test_stage_artefacts_appends_github_output(
    stage_common: object, workspace: Path
) -> None:
    """Writing outputs should append to the existing ``GITHUB_OUTPUT`` file."""

    target = "x86_64-unknown-linux-gnu"
    write_workspace_inputs(workspace, target)

    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="LICENSE",
                required=True,
                output="license_path",
            ),
        ],
        platform="linux",
        arch="amd64",
        target=target,
    )

    github_output = workspace / "outputs.txt"
    github_output.write_text("previous=value\n", encoding="utf-8")

    stage_common.stage_artefacts(config, github_output)

    content = github_output.read_text(encoding="utf-8")
    assert content.startswith("previous=value"), "Existing lines should remain at the top"
    assert "artifact_dir=" in content, "New outputs should be appended to the file"


def test_stage_artefacts_uses_alternative_glob(
    stage_common: object, workspace: Path
) -> None:
    """Fallback paths should be used when the preferred template is absent."""
    target = "x86_64-unknown-linux-gnu"
    write_workspace_inputs(workspace, target)
    generated = (
        workspace / "target" / "generated-man" / target / "release" / "netsuke.1"
    )
    generated.unlink()

    build_dir = workspace / "target" / target / "release" / "build"
    first = build_dir / "1" / "out" / "netsuke.1"
    second = build_dir / "2" / "out" / "netsuke.1"
    first.parent.mkdir(parents=True, exist_ok=True)
    second.parent.mkdir(parents=True, exist_ok=True)
    first.write_text(".TH 1", encoding="utf-8")
    second.write_text(".TH 2", encoding="utf-8")
    os.utime(first, (first.stat().st_atime, first.stat().st_mtime - 100))

    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="target/generated-man/{target}/release/{bin_name}.1",
                required=True,
                output="man_path",
                alternatives=["target/{target}/release/build/*/out/{bin_name}.1"],
            ),
        ],
        platform="linux",
        arch="amd64",
        target=target,
    )

    github_output = workspace / "outputs.txt"
    result = stage_common.stage_artefacts(config, github_output)
    staged_path = result.outputs["man_path"]
    assert (
        staged_path.read_text(encoding="utf-8") == ".TH 2"
    ), "Fallback glob should pick the newest man page"


def test_stage_artefacts_glob_selects_newest_candidate(
    stage_common: object, workspace: Path
) -> None:
    """Glob matches should resolve to the most recently modified file."""
    target = "x86_64-unknown-linux-gnu"
    write_workspace_inputs(workspace, target)
    generated = (
        workspace / "target" / "generated-man" / target / "release" / "netsuke.1"
    )
    generated.unlink()

    build_dir = workspace / "target" / target / "release" / "build"
    build_dir.mkdir(parents=True, exist_ok=True)
    candidates = []
    for idx in range(3):
        candidate = build_dir / f"{idx}" / "out" / "netsuke.1"
        candidate.parent.mkdir(parents=True, exist_ok=True)
        candidate.write_text(f".TH {idx}", encoding="utf-8")
        os.utime(candidate, (100 + idx, 100 + idx))
        candidates.append(candidate)

    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="target/generated-man/{target}/release/{bin_name}.1",
                required=True,
                output="man_path",
                alternatives=["target/{target}/release/build/*/out/{bin_name}.1"],
            ),
        ],
        platform="linux",
        arch="amd64",
        target=target,
    )

    github_output = workspace / "outputs.txt"
    result = stage_common.stage_artefacts(config, github_output)
    staged_path = result.outputs["man_path"]
    assert (
        staged_path.read_text(encoding="utf-8") == ".TH 2"
    ), "Glob resolution should select the most recent candidate"
    latest = max(candidates, key=lambda f: f.stat().st_mtime_ns)
    assert (
        latest.read_text(encoding="utf-8") == staged_path.read_text(encoding="utf-8")
    ), "Selected candidate should match the most recent file"


def test_match_candidate_path_handles_windows_drive(
    stage_common: object, workspace: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Absolute Windows-style globs should resolve relative to the drive root."""
    monkeypatch.chdir(workspace)

    drive_root = Path("C:\\")
    windows_workspace = drive_root / "workspace"
    man_dir = windows_workspace / "man"
    man_dir.mkdir(parents=True, exist_ok=True)
    candidate = man_dir / "netsuke.1"
    candidate.write_text(".TH WINDOWS", encoding="utf-8")

    staging = importlib.import_module("stage_common.staging")
    matched = staging._match_candidate_path(
        windows_workspace, "C:/workspace/man/*.1"
    )

    assert matched == candidate


def test_stage_artefacts_warns_for_optional(
    stage_common: object, workspace: Path, capfd: pytest.CaptureFixture[str]
) -> None:
    """Optional artefacts should emit a warning when absent but not abort."""
    target = "x86_64-unknown-linux-gnu"
    write_workspace_inputs(workspace, target)

    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="target/{target}/release/{bin_name}{bin_ext}",
                required=True,
            ),
            stage_common.ArtefactConfig(
                source="missing.txt",
                required=False,
                output="missing",
            ),
        ],
        platform="linux",
        arch="amd64",
        target=target,
    )

    stage_common.stage_artefacts(config, workspace / "out.txt")
    captured = capfd.readouterr()
    assert (
        "::warning title=Artefact Skipped::Optional artefact missing" in captured.err
    ), "Optional artefact warning missing"


def test_stage_artefacts_fails_with_attempt_context(
    stage_common: object, workspace: Path
) -> None:
    """Missing required artefacts should include context in the error message."""
    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[
            stage_common.ArtefactConfig(
                source="missing-{target}",
                required=True,
            ),
        ],
        platform="linux",
        arch="amd64",
        target="x86_64-unknown-linux-gnu",
    )

    with pytest.raises(stage_common.StageError) as exc:
        stage_common.stage_artefacts(config, workspace / "outputs.txt")

    message = str(exc.value)
    assert "Workspace=" in message, "Workspace context missing from error"
    assert "missing-{target}" in message, "Template pattern missing from error"
    assert (
        "missing-x86_64-unknown-linux-gnu" in message
    ), "Rendered path missing from error"


def test_glob_root_and_pattern_handles_windows_drive(stage_common: object) -> None:
    """Absolute Windows globs should strip the drive before globbing."""

    staging = importlib.import_module("stage_common.staging")
    helper = staging._glob_root_and_pattern

    root, pattern = helper(PureWindowsPath("C:/dist/*.zip"))
    assert root == "C:\\"
    assert pattern == "dist/*.zip"


def test_glob_root_and_pattern_returns_wildcard_for_root_only(
    stage_common: object,
) -> None:
    """Root-only absolute paths should glob all children."""

    staging = importlib.import_module("stage_common.staging")
    helper = staging._glob_root_and_pattern

    root, pattern = helper(PureWindowsPath("C:/"))
    assert root == "C:\\"
    assert pattern == "*"


def test_glob_root_and_pattern_handles_posix_absolute(stage_common: object) -> None:
    """POSIX absolute paths should preserve relative segments for globbing."""

    staging = importlib.import_module("stage_common.staging")
    helper = staging._glob_root_and_pattern

    root, pattern = helper(PurePosixPath("/tmp/dist/*.zip"))
    assert root == "/"
    assert pattern.endswith("dist/*.zip"), pattern


def test_glob_root_and_pattern_rejects_relative_paths(stage_common: object) -> None:
    """Relative globs should be rejected to avoid ambiguous anchors."""

    staging = importlib.import_module("stage_common.staging")
    helper = staging._glob_root_and_pattern

    with pytest.raises(ValueError):
        helper(PurePosixPath("dist/*.zip"))


def test_stage_artefacts_matches_absolute_glob(
    stage_common: object, workspace: Path
) -> None:
    """Absolute glob patterns should allow staging artefacts."""

    absolute_root = workspace / "absolute" / "1.2.3"
    absolute_root.mkdir(parents=True, exist_ok=True)
    source_path = absolute_root / "netsuke.txt"
    source_path.write_text("payload", encoding="utf-8")

    artefact = stage_common.ArtefactConfig(
        source=f"{workspace.as_posix()}/absolute/*/netsuke.txt",
        required=True,
        output="absolute_path",
    )
    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[artefact],
        platform="linux",
        arch="amd64",
        target="x86_64-unknown-linux-gnu",
    )

    github_output = workspace / "github_output.txt"
    result = stage_common.stage_artefacts(config, github_output)

    assert result.staged_artefacts, "Expected artefact to be staged from absolute glob"
    staged_path = result.staged_artefacts[0]
    assert staged_path.read_text(encoding="utf-8") == "payload"
    assert result.outputs["absolute_path"] == staged_path
