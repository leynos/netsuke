"""Configuration loader tests for the staging helper."""

from __future__ import annotations

from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parent.parent


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
    ("config_content", "error_substring", "test_id"),
    [
        pytest.param(
            """\
[common]
bin_name = \"netsuke\"
checksum_algorithm = \"unknown\"
artefacts = [ { source = \"LICENSE\" } ]

[targets.test]
platform = \"linux\"
arch = \"amd64\"
target = \"x86_64-unknown-linux-gnu\"
""",
            "Unsupported checksum algorithm",
            "rejects_unknown_checksum",
            id="rejects_unknown_checksum",
        ),
        pytest.param(
            """\
[common]
checksum_algorithm = \"sha256\"
artefacts = [ { source = \"LICENSE\" } ]

[targets.test]
arch = \"amd64\"
target = \"x86_64-unknown-linux-gnu\"
platform = \"linux\"
""",
            "bin_name",
            "requires_common_bin_name",
            id="requires_common_bin_name",
        ),
        pytest.param(
            """\
[common]
bin_name = \"netsuke\"
checksum_algorithm = \"sha256\"
artefacts = [ { source = \"LICENSE\" } ]

[targets.test]
arch = \"amd64\"
target = \"x86_64-unknown-linux-gnu\"
""",
            "platform",
            "requires_target_platform",
            id="requires_target_platform",
        ),
        pytest.param(
            """\
[common]
bin_name = \"netsuke\"
checksum_algorithm = \"sha256\"
artefacts = [ { output = \"binary\" } ]

[targets.test]
platform = \"linux\"
arch = \"amd64\"
target = \"x86_64-unknown-linux-gnu\"
""",
            "source",
            "requires_artefact_source",
            id="requires_artefact_source",
        ),
        pytest.param(
            """\
[common]
bin_name = \"netsuke\"
checksum_algorithm = \"sha256\"
artefacts = [ { source = \"LICENSE\" } ]

[targets.other]
platform = \"linux\"
arch = \"amd64\"
target = \"x86_64-unknown-linux-gnu\"
""",
            "Missing configuration key",
            "requires_target_section",
            id="requires_target_section",
        ),
    ],
)
def test_load_config_validation_errors(
    stage_common: object,
    workspace: Path,
    config_content: str,
    error_substring: str,
    test_id: str,
) -> None:
    """``load_config`` should raise ``StageError`` for invalid configurations."""

    config_file = workspace / "release-staging.toml"
    config_file.write_text(config_content, encoding="utf-8")

    with pytest.raises(stage_common.StageError) as exc:
        stage_common.load_config(config_file, "test")

    message = str(exc.value)
    assert error_substring in message, f"{test_id} missing expected substring"

    if test_id == "requires_common_bin_name":
        assert "[common]" in message
    elif test_id == "requires_target_platform":
        assert "[targets.test]" in message
    elif test_id == "requires_artefact_source":
        assert "entry #1" in message
