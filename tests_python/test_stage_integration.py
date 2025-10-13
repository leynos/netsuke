"""Behavioural staging tests covering integration flows end-to-end."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from stage_test_helpers import decode_output_file, write_workspace_inputs


class TestSuccessfulRuns:
    """Integration scenarios where staging succeeds."""

    def test_exports_metadata(self, stage_common: object, workspace: Path) -> None:
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
        assert (
            result.staging_dir == staging_dir
        ), "StageResult must record the staging directory"
        assert staging_dir.exists(), "Expected staging directory to be created"

        staged_files = {path.name for path in result.staged_artefacts}
        assert staged_files == {
            "netsuke",
            "netsuke.1",
            "LICENSE",
        }, "Unexpected artefacts staged"
        assert set(result.outputs) == {
            "binary_path",
            "man_path",
            "license_path",
        }, "Outputs missing expected keys"
        expected_checksums = {
            "netsuke": staging_dir / "netsuke.sha256",
            "netsuke.1": staging_dir / "netsuke.1.sha256",
            "LICENSE": staging_dir / "LICENSE.sha256",
        }
        assert set(result.checksums) == set(
            expected_checksums
        ), "Checksum outputs missing entries"
        for path in expected_checksums.values():
            assert path.exists(), f"Checksum file {path.name} was not written"

        outputs = decode_output_file(github_output)
        assert (
            outputs["artifact_dir"] == staging_dir.as_posix()
        ), "artifact_dir output should reference staging directory"
        assert (
            outputs["binary_path"].endswith("netsuke")
        ), "binary_path output should point to the staged executable"
        assert (
            outputs["license_path"].endswith("LICENSE")
        ), "license_path output should point to the staged licence"
        assert (
            outputs["dist_dir"].endswith("dist")
        ), "dist_dir output should reflect parent directory"
        staged_listing = outputs["staged_files"].splitlines()
        assert staged_listing == sorted(
            staged_listing
        ), "Staged files output should be sorted"
        artefact_map = json.loads(outputs["artefact_map"])
        assert artefact_map["binary_path"].endswith(
            "netsuke"
        ), "artefact map should include the binary path"
        assert artefact_map["license_path"].endswith(
            "LICENSE"
        ), "artefact map should include the licence path"
        checksum_map = json.loads(outputs["checksum_map"])
        assert set(checksum_map) == {
            "netsuke",
            "netsuke.1",
            "LICENSE",
        }, "Checksum map missing entries"

    def test_reinitialises_staging_dir(
        self, stage_common: object, workspace: Path
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
        assert (
            "obsolete.txt" not in current_entries
        ), "Old entries must not survive reinitialisation"

    def test_appends_github_output(
        self, stage_common: object, workspace: Path
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
        assert content.startswith(
            "previous=value"
        ), "Existing lines should remain at the top"
        assert "artifact_dir=" in content, "New outputs should be appended to the file"

    def test_warns_for_optional(
        self,
        stage_common: object,
        workspace: Path,
        caplog: pytest.LogCaptureFixture,
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

        with caplog.at_level("WARNING"):
            stage_common.stage_artefacts(config, workspace / "out.txt")

        assert any(
            "Optional artefact missing" in message for message in caplog.messages
        ), "Optional artefact warning missing"

    def test_honours_destination_templates(
        self, stage_common: object, workspace: Path
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
        assert (
            relative.as_posix() == "artifacts/netsuke/payload.bin"
        ), (
            "staged path relative to staging_dir should be "
            "'artifacts/netsuke/payload.bin' "
            f"(got: {relative.as_posix()})"
        )


class TestFailureModes:
    """Integration scenarios covering failure cases."""

    def test_rejects_reserved_output_key(
        self, stage_common: object, workspace: Path
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
        assert (
            github_output.read_text(encoding="utf-8") == ""
        ), "Outputs should not be written when validation fails"

    def test_fails_with_attempt_context(
        self, stage_common: object, workspace: Path
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
