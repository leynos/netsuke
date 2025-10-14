"""Tests exercising staging helper utilities and iterators."""

from __future__ import annotations

from pathlib import Path

import pytest


class TestInitializeStagingDir:
    """Tests covering staging directory initialisation."""

    def test_removes_existing_contents(
        self, staging_pipeline: object, tmp_path: Path
    ) -> None:
        """The helper should clear any previous staging directory contents."""

        staging_dir = tmp_path / "stage"
        stale_file = staging_dir / "stale.txt"
        stale_file.parent.mkdir(parents=True, exist_ok=True)
        stale_file.write_text("old", encoding="utf-8")

        staging_pipeline._initialize_staging_dir(staging_dir)

        assert staging_dir.exists(), "Expected staging directory to be recreated"
        assert not list(staging_dir.iterdir()), "Stale artefacts should be removed"


class TestIterStagedArtefacts:
    """Tests for the private iterator driving staging."""

    def test_yields_metadata(
        self, stage_common: object, staging_pipeline: object, workspace: Path
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
        staging_pipeline._initialize_staging_dir(staging_dir)

        staged = list(
            staging_pipeline._iter_staged_artefacts(config, staging_dir, context)
        )

        assert len(staged) == 1, "Expected the iterator to yield the staged artefact"
        entry = staged[0]
        assert isinstance(
            entry, staging_pipeline.StagedArtefact
        ), "Expected staged entry to be a StagedArtefact"
        assert entry.path.exists(), "Staged artefact path should exist on disk"
        assert entry.checksum, "Iterator should include a checksum digest"
        checksum_file = entry.path.with_name(f"{entry.path.name}.sha256")
        assert checksum_file.exists(), "Checksum sidecar should be written"

    def test_aligns_with_public_result(
        self, stage_common: object, staging_pipeline: object, workspace: Path
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
        staging_pipeline._initialize_staging_dir(staging_dir)
        iter_names = [
            entry.path.name
            for entry in staging_pipeline._iter_staged_artefacts(
                config, staging_dir, context
            )
        ]

        github_output = workspace / "outputs.txt"
        result = stage_common.stage_artefacts(config, github_output)

        expected_names = [path.name for path in result.staged_artefacts]
        assert (
            iter_names == expected_names
        ), f"Expected iterator names {expected_names}, got {iter_names}"


class TestStageSingleArtefact:
    """Tests covering the single artefact staging helper."""

    def test_overwrites_existing_file(
        self, stage_common: object, staging_pipeline: object, workspace: Path
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
        env = staging_pipeline._StagingEnvironment(
            staging_dir=staging_dir,
            context=context,
        )
        staged_path = staging_pipeline._stage_single_artefact(
            config, env, artefact, source
        )

        assert (
            staged_path == stale
        ), "Staged artefact path should reuse the existing destination"
        assert (
            staged_path.read_text(encoding="utf-8") == "new"
        ), "Staged artefact content should be overwritten with the new payload"


class TestEnsureSourceAvailable:
    """Tests for `_ensure_source_available` covering required and optional paths."""

    @pytest.mark.parametrize(
        "source",
        [
            pytest.param("missing.bin", id="normal_path"),
            pytest.param("payload\x00bin", id="invalid_characters"),
        ],
    )
    def test_required_error(
        self,
        stage_common: object,
        staging_pipeline: object,
        workspace: Path,
        source: str,
    ) -> None:
        """Missing required artefacts should raise a StageError for both normal and invalid paths."""

        artefact = stage_common.ArtefactConfig(source=source, required=True)

        self._assert_required_error(
            stage_common,
            staging_pipeline,
            workspace,
            artefact,
        )

    def test_optional_warning(
        self,
        stage_common: object,
        staging_pipeline: object,
        workspace: Path,
        caplog: pytest.LogCaptureFixture,
    ) -> None:
        """Optional artefacts should be skipped with a warning instead of failing."""

        artefact = stage_common.ArtefactConfig(source="missing.txt", required=False)

        with caplog.at_level("WARNING"):
            should_stage = staging_pipeline._ensure_source_available(
                None,
                artefact,
                [staging_pipeline._RenderAttempt("missing.txt", "missing.txt")],
                workspace,
            )

        assert not should_stage, "Optional artefacts should not be staged"
        assert any(
            "missing.txt" in message for message in caplog.messages
        ), "Expected warning to mention missing optional artefact 'missing.txt'"

    def _assert_required_error(
        self,
        stage_common: object,
        staging_pipeline: object,
        workspace: Path,
        artefact: object,
    ) -> None:
        """Assert that missing required artefacts raise informative StageErrors."""

        source = artefact.source
        attempts = [
            staging_pipeline._RenderAttempt(source, source),
        ]

        with pytest.raises(stage_common.StageError) as exc:
            staging_pipeline._ensure_source_available(
                None, artefact, attempts, workspace
            )

        message = str(exc.value)
        assert (
            "Required artefact not found" in message
        ), "Missing error preamble for required artefact"
        assert source in message, (
            "Missing artefact source in error message: expected to find "
            f"'{source}'"
        )
