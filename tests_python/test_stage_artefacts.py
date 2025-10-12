"""Tests exercising artefact staging and output generation."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from stage_test_helpers import decode_output_file, write_workspace_inputs


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


def test_iter_staged_artefacts_yields_metadata(
    stage_common: object, staging_module: object, workspace: Path
) -> None:
    """The iterator should yield dataclass entries with staged file metadata."""

    source = workspace / "LICENSE"
    source.write_text("payload", encoding="utf-8")
    artefact = stage_common.ArtefactConfig(source="LICENSE", required=True)
    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[artefact],
        platform="linux",
        arch="amd64",
        target="test",
    )

    staging_dir = config.staging_dir()
    context = config.as_template_context()
    staging_module._initialize_staging_dir(staging_dir)

    staged = list(
        staging_module._iter_staged_artefacts(config, staging_dir, context)
    )

    assert len(staged) == 1, "Expected the iterator to yield the staged artefact"
    entry = staged[0]
    assert isinstance(entry, staging_module.StagedArtefact)
    assert entry.path.exists(), "Staged artefact path should exist on disk"
    assert entry.checksum, "Iterator should include a checksum digest"
    checksum_file = entry.path.with_name(f"{entry.path.name}.sha256")
    assert checksum_file.exists(), "Checksum sidecar should be written"


def test_stage_artefacts_aligns_with_iterator(
    stage_common: object, staging_module: object, workspace: Path
) -> None:
    """Behaviourally verify the iterator matches the public staging result."""

    (workspace / "first.txt").write_text("first", encoding="utf-8")
    (workspace / "second.txt").write_text("second", encoding="utf-8")
    artefacts = [
        stage_common.ArtefactConfig(source="first.txt", required=True),
        stage_common.ArtefactConfig(source="second.txt", required=True),
    ]
    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=artefacts,
        platform="linux",
        arch="amd64",
        target="behavioural",
    )

    staging_dir = config.staging_dir()
    context = config.as_template_context()
    staging_module._initialize_staging_dir(staging_dir)
    iter_names = [
        entry.path.name
        for entry in staging_module._iter_staged_artefacts(
            config, staging_dir, context
        )
    ]

    github_output = workspace / "outputs.txt"
    result = stage_common.stage_artefacts(config, github_output)

    assert iter_names == [path.name for path in result.staged_artefacts]


def test_stage_single_artefact_overwrites_existing_file(
    stage_common: object, staging_module: object, workspace: Path
) -> None:
    """The helper should replace existing staged files atomically."""

    source = workspace / "binary"
    source.write_text("new", encoding="utf-8")
    artefact = stage_common.ArtefactConfig(
        source="binary",
        destination="bin/{source_name}",
        required=True,
    )
    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[artefact],
        platform="linux",
        arch="amd64",
        target="unit",
    )

    staging_dir = config.staging_dir()
    stale = staging_dir / "bin" / "binary"
    stale.parent.mkdir(parents=True, exist_ok=True)
    stale.write_text("old", encoding="utf-8")

    context = config.as_template_context()
    env = staging_module._StagingEnvironment(
        staging_dir=staging_dir,
        context=context,
    )
    staged_path = staging_module._stage_single_artefact(
        config, env, artefact, source
    )

    assert staged_path == stale
    assert staged_path.read_text(encoding="utf-8") == "new"


def test_stage_artefacts_honours_destination_templates(
    stage_common: object, workspace: Path
) -> None:
    """Destination templates should be rendered beneath the staging directory."""

    source = workspace / "payload.bin"
    source.write_text("payload", encoding="utf-8")
    artefact = stage_common.ArtefactConfig(
        source="payload.bin",
        destination="artifacts/{bin_name}/{source_name}",
        required=True,
        output="payload_path",
    )
    config = stage_common.StagingConfig(
        workspace=workspace,
        bin_name="netsuke",
        dist_dir="dist",
        checksum_algorithm="sha256",
        artefacts=[artefact],
        platform="linux",
        arch="amd64",
        target="behavioural",
    )

    github_output = workspace / "github.txt"
    result = stage_common.stage_artefacts(config, github_output)

    staged_path = result.outputs["payload_path"]
    relative = staged_path.relative_to(result.staging_dir)
    assert relative.as_posix() == "artifacts/netsuke/payload.bin"


def test_ensure_source_available_required_error(
    stage_common: object, staging_module: object, workspace: Path
) -> None:
    """Missing required artefacts should raise a StageError with context."""

    artefact = stage_common.ArtefactConfig(source="missing.bin", required=True)
    attempts = [
        staging_module._RenderAttempt("missing.bin", "missing.bin"),
    ]

    with pytest.raises(stage_common.StageError) as exc:
        staging_module._ensure_source_available(
            None, artefact, attempts, workspace
        )

    message = str(exc.value)
    assert "Required artefact not found" in message
    assert "missing.bin" in message


def test_ensure_source_available_optional_warning(
    stage_common: object,
    staging_module: object,
    workspace: Path,
    capfd: pytest.CaptureFixture[str],
) -> None:
    """Optional artefacts should be skipped with a warning instead of failing."""

    artefact = stage_common.ArtefactConfig(source="missing.txt", required=False)

    should_stage = staging_module._ensure_source_available(
        None,
        artefact,
        [staging_module._RenderAttempt("missing.txt", "missing.txt")],
        workspace,
    )

    captured = capfd.readouterr()
    assert not should_stage
    assert "Optional artefact missing" in captured.err
