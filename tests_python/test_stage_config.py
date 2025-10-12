"""Tests covering configuration loading and validation for staging."""

from __future__ import annotations

from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parent.parent


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


@pytest.mark.parametrize(
    ("test_id", "toml_content", "target_key", "expected_substrings"),
    [
        pytest.param(
            "unknown_checksum",
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
            "test",
            ["Unsupported checksum algorithm"],
            id="unknown_checksum",
        ),
        pytest.param(
            "missing_common_bin_name",
            """\
[common]
checksum_algorithm = "sha256"
artefacts = [ { source = "LICENSE" } ]

[targets.test]
arch = "amd64"
target = "x86_64-unknown-linux-gnu"
platform = "linux"
""",
            "test",
            ["bin_name", "[common]"],
            id="missing_common_bin_name",
        ),
        pytest.param(
            "missing_target_platform",
            """\
[common]
bin_name = "netsuke"
checksum_algorithm = "sha256"
artefacts = [ { source = "LICENSE" } ]

[targets.test]
arch = "amd64"
target = "x86_64-unknown-linux-gnu"
""",
            "test",
            ["platform", "[targets.test]"],
            id="missing_target_platform",
        ),
        pytest.param(
            "missing_artefact_source",
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
            "test",
            ["source", "entry #1"],
            id="missing_artefact_source",
        ),
        pytest.param(
            "missing_target_section",
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
            "test",
            ["Missing configuration key"],
            id="missing_target_section",
        ),
    ],
)
def test_load_config_validation_errors(
    test_id: str,
    toml_content: str,
    target_key: str,
    expected_substrings: list[str],
    stage_common: object,
    workspace: Path,
) -> None:
    """``load_config`` should surface friendly validation errors."""
    config_file = workspace / "release-staging.toml"
    config_file.write_text(toml_content, encoding="utf-8")

    with pytest.raises(stage_common.StageError) as exc:
        stage_common.load_config(config_file, target_key)

    message = str(exc.value)
    for substring in expected_substrings:
        assert substring in message, f"{test_id} missing substring: {substring!r}"
