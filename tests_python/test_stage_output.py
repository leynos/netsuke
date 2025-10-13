"""Tests covering staging output formatting and validation helpers."""

from __future__ import annotations

import json
from pathlib import Path

import pytest


class TestPrepareOutputData:
    """Tests for the ``_prepare_output_data`` helper."""

    def test_returns_sorted_metadata(
        self, staging_output: object, tmp_path: Path
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

        result = staging_output._prepare_output_data(
            staging_dir, staged, outputs, checksums
        )

        assert result["artifact_dir"].endswith(
            "stage"
        ), "Expected staging directory output"
        assert result["dist_dir"].endswith(
            "dist"
        ), "Expected dist directory output"
        assert result["staged_files"].splitlines() == [
            "a.txt",
            "b.bin",
        ], "Staged files should be sorted"
        artefact_map = json.loads(result["artefact_map"])
        assert list(artefact_map) == [
            "binary",
            "manual",
        ], "Outputs should be sorted"
        checksum_map = json.loads(result["checksum_map"])
        assert list(checksum_map) == [
            "a.txt",
            "b.bin",
        ], "Checksums should be sorted"


class TestValidateReservedKeys:
    """Tests validating reserved GitHub output key handling."""

    def test_rejects_reserved_keys(
        self, staging_output: object, tmp_path: Path
    ) -> None:
        """Reserved workflow keys should trigger a stage error."""

        staging_dir = tmp_path / "stage"
        with pytest.raises(staging_output.StageError) as exc:
            staging_output._validate_no_reserved_key_collisions(
                {"artifact_dir": staging_dir}
            )

        assert "collide with reserved keys" in str(exc.value)


class TestWriteGithubOutput:
    """Tests covering the GitHub output writer."""

    def test_formats_empty_values(
        self, staging_output: object, tmp_path: Path
    ) -> None:
        """The helper should handle empty strings and lists without errors."""

        output_file = tmp_path / "github" / "output.txt"
        staging_output.write_github_output(
            output_file,
            {
                "empty_str": "",
                "empty_list": [],
            },
        )

        content = output_file.read_text(encoding="utf-8")
        lines = content.splitlines()
        assert lines[0] == "empty_list<<gh_EMPTY_LIST"
        assert lines[1] == ""
        assert lines[2] == "gh_EMPTY_LIST"
        assert lines[3] == "empty_str="

    def test_formats_values(
        self, staging_output: object, tmp_path: Path
    ) -> None:
        """The GitHub output helper should escape strings and stream lists."""

        output_file = tmp_path / "github" / "output.txt"
        output_file.parent.mkdir(parents=True, exist_ok=True)
        output_file.write_text("initial=value\n", encoding="utf-8")

        staging_output.write_github_output(
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
        lines = content.splitlines()
        lines_header_index = lines.index("lines<<gh_LINES")
        name_index = next(
            idx for idx, value in enumerate(lines) if value.startswith("name=")
        )
        assert (
            lines_header_index < name_index
        ), "Outputs should be written in deterministic sorted order"
