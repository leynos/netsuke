"""Artefact staging logic shared across the CLI and composite action.

This module provides the core staging pipeline that copies artefacts from a
workspace into a staging directory, computes checksums, and exports outputs for
GitHub Actions workflows.

Usage
-----
From the CLI entry point::

    import os
    from pathlib import Path
    from stage_common import load_config, stage_artefacts

    config = load_config(Path(".github/release-staging.toml"), "linux-x86_64")
    result = stage_artefacts(config, Path(os.environ["GITHUB_OUTPUT"]))
    print(f"Staged {len(result.staged_artefacts)} artefacts.")
"""

from __future__ import annotations

import dataclasses
import json
import shutil
import sys
import typing as typ
from pathlib import Path

from .checksum_utils import write_checksum
from .errors import StageError
from .fs_utils import safe_destination_path
from .github_output import write_github_output
from .template_utils import render_template, resolve_artefact_source

if typ.TYPE_CHECKING:
    from .config import ArtefactConfig, StagingConfig

RESERVED_OUTPUT_KEYS = {
    "artifact_dir",
    "dist_dir",
    "staged_files",
    "artefact_map",
    "checksum_map",
}

__all__ = ["RESERVED_OUTPUT_KEYS", "StageResult", "stage_artefacts"]


@dataclasses.dataclass(slots=True)
class StageResult:
    """Outcome of :func:`stage_artefacts`."""

    staging_dir: Path
    staged_artefacts: list[Path]
    outputs: dict[str, Path]
    checksums: dict[str, str]


@dataclasses.dataclass(slots=True)
class _StageOutcome:
    """Result of staging a single artefact."""

    path: Path
    output_key: str | None
    digest: str


def stage_artefacts(config: "StagingConfig", github_output_file: Path) -> StageResult:
    """Copy artefacts into ``config``'s staging directory.

    Parameters
    ----------
    config : StagingConfig
        Fully resolved configuration describing the artefacts to stage.
    github_output_file : Path
        Path to the ``GITHUB_OUTPUT`` file used to export workflow outputs.

    Returns
    -------
    StageResult
        Summary object describing the staging directory, staged artefacts,
        exported outputs, and checksum digests.

    Raises
    ------
    StageError
        Raised when required artefacts are missing or configuration templates
        render invalid destinations.
    """
    staging_dir = config.staging_dir()
    context = config.as_template_context()

    if staging_dir.exists():
        shutil.rmtree(staging_dir)
    staging_dir.mkdir(parents=True, exist_ok=True)

    staged_paths: list[Path] = []
    outputs: dict[str, Path] = {}
    checksums: dict[str, str] = {}

    for artefact in config.artefacts:
        outcome = _stage_single_artefact(
            config=config,
            artefact=artefact,
            staging_dir=staging_dir,
            context=context,
        )
        if outcome is None:
            continue
        staged_paths.append(outcome.path)
        checksums[outcome.path.name] = outcome.digest
        if outcome.output_key:
            outputs[outcome.output_key] = outcome.path

    if not staged_paths:
        message = "No artefacts were staged."
        raise StageError(message)

    staged_file_names = [path.name for path in sorted(staged_paths)]
    artefact_map_json = json.dumps(
        {key: path.as_posix() for key, path in sorted(outputs.items())}
    )
    checksum_map_json = json.dumps(dict(sorted(checksums.items())))

    if colliding_keys := RESERVED_OUTPUT_KEYS & outputs.keys():
        message = (
            "Artefact outputs collide with reserved keys: "
            f"{sorted(colliding_keys)}. Please rename these keys in your "
            "artefact configuration to avoid using reserved names: "
            f"{sorted(RESERVED_OUTPUT_KEYS)}."
        )
        raise StageError(message)

    exported_outputs: dict[str, str | list[str]] = {
        "artifact_dir": staging_dir.as_posix(),
        "dist_dir": staging_dir.parent.as_posix(),
        "staged_files": staged_file_names,
        "artefact_map": artefact_map_json,
        "checksum_map": checksum_map_json,
    } | {key: path.as_posix() for key, path in outputs.items()}

    write_github_output(github_output_file, exported_outputs)

    return StageResult(staging_dir, staged_paths, outputs, checksums)


def _stage_single_artefact(
    *,
    config: "StagingConfig",
    artefact: "ArtefactConfig",
    staging_dir: Path,
    context: dict[str, typ.Any],
) -> _StageOutcome | None:
    """Stage ``artefact`` and return its outcome."""

    source_path, attempts = resolve_artefact_source(
        config.workspace, artefact, context
    )
    if source_path is None:
        if artefact.required:
            attempt_lines = ", ".join(
                f"{attempt.template!r} -> {attempt.rendered!r}" for attempt in attempts
            )
            message = (
                "Required artefact not found. "
                f"Workspace={config.workspace.as_posix()} "
                f"Attempts=[{attempt_lines}]"
            )
            raise StageError(message)
        warning = (
            "::warning title=Artefact Skipped::Optional artefact missing: "
            f"{artefact.source}"
        )
        print(warning, file=sys.stderr)
        return None

    artefact_context = context | {
        "source_path": source_path.as_posix(),
        "source_name": source_path.name,
    }
    destination_text = (
        render_template(destination, artefact_context)
        if (destination := artefact.destination)
        else source_path.name
    )

    destination_path = safe_destination_path(staging_dir, destination_text)
    if destination_path.exists():
        destination_path.unlink()
    shutil.copy2(source_path, destination_path)
    print(
        f"Staged '{source_path.relative_to(config.workspace)}' ->"
        f" '{destination_path.relative_to(config.workspace)}'",
    )

    digest = write_checksum(destination_path, config.checksum_algorithm)

    return _StageOutcome(
        path=destination_path,
        output_key=artefact.output,
        digest=digest,
    )
