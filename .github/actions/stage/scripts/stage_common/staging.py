"""Artefact staging logic shared across the CLI and composite action.

This module provides the core staging pipeline that copies artefacts from a
workspace into a staging directory, computes checksums, and exports outputs
for GitHub Actions workflows.

Usage
-----
From the CLI entry point::

    import os
    from pathlib import Path

    from stage_common import load_config, stage_artefacts

    config = load_config(Path(".github/release-staging.toml"), "linux-x86_64")
    result = stage_artefacts(config, Path(os.environ["GITHUB_OUTPUT"]))
    print(f"Staged {len(result.staged_artefacts)} artefacts.")

From the composite action::

    - name: Stage artefacts
      uses: ./.github/actions/stage
      with:
        config-file: .github/release-staging.toml
        target: linux-x86_64
"""

from __future__ import annotations

import dataclasses
import hashlib
import json
import shutil
import sys
import typing as typ
from pathlib import Path, PurePosixPath, PureWindowsPath

from .errors import StageError

if typ.TYPE_CHECKING:
    from pathlib import PurePath

    from .config import ArtefactConfig, StagingConfig

__all__ = ["StageResult", "stage_artefacts"]


RESERVED_OUTPUT_KEYS: set[str] = {
    "artifact_dir",
    "dist_dir",
    "staged_files",
    "artefact_map",
    "checksum_map",
}


@dataclasses.dataclass(slots=True)
class _RenderAttempt:
    template: str
    rendered: str


@dataclasses.dataclass(slots=True)
class StageResult:
    """Outcome of :func:`stage_artefacts`."""

    staging_dir: Path
    staged_artefacts: list[Path]
    outputs: dict[str, Path]
    checksums: dict[str, str]


def _initialize_staging_dir(staging_dir: Path) -> None:
    """Create a clean staging directory ready to receive artefacts.

    Parameters
    ----------
    staging_dir : Path
        Absolute path to the staging directory that will hold staged
        artefacts.

    Examples
    --------
    >>> staging_dir = Path("/tmp/stage")
    >>> (staging_dir / "old").mkdir(parents=True, exist_ok=True)
    >>> _initialize_staging_dir(staging_dir)
    >>> staging_dir.exists()
    True
    """

    if staging_dir.exists():
        shutil.rmtree(staging_dir)
    staging_dir.mkdir(parents=True, exist_ok=True)


def _prepare_output_data(
    staging_dir: Path,
    staged_paths: list[Path],
    outputs: dict[str, Path],
    checksums: dict[str, str],
) -> dict[str, str | list[str]]:
    """Assemble workflow outputs describing the staged artefacts.

    Parameters
    ----------
    staging_dir : Path
        Directory that now contains all staged artefacts.
    staged_paths : list[Path]
        Collection of artefact paths copied into ``staging_dir``.
    outputs : dict[str, Path]
        Mapping of configured GitHub Action output keys to staged artefact
        destinations.
    checksums : dict[str, str]
        Mapping of staged artefact file names to their checksum digests.

    Returns
    -------
    dict[str, str | list[str]]
        Dictionary describing the staging results ready to be exported to the
        GitHub Actions output file.

    Examples
    --------
    >>> staging_dir = Path("/tmp/stage")
    >>> staged = [staging_dir / "bin.tar.gz"]
    >>> outputs = {"archive": staged[0]}
    >>> checksums = {"bin.tar.gz": "abc123"}
    >>> result = _prepare_output_data(staging_dir, staged, outputs, checksums)
    >>> sorted(result)
    ['archive', 'artefact_map', 'artifact_dir', 'checksum_map', 'dist_dir',
     'staged_files']
    """

    staged_file_names = [path.name for path in sorted(staged_paths)]
    artefact_map_json = json.dumps(
        {key: path.as_posix() for key, path in sorted(outputs.items())}
    )
    checksum_map_json = json.dumps(dict(sorted(checksums.items())))

    return {
        "artifact_dir": staging_dir.as_posix(),
        "dist_dir": staging_dir.parent.as_posix(),
        "staged_files": "\n".join(staged_file_names),
        "artefact_map": artefact_map_json,
        "checksum_map": checksum_map_json,
    } | {key: path.as_posix() for key, path in outputs.items()}


def _validate_no_reserved_key_collisions(outputs: dict[str, Path]) -> None:
    """Ensure user-defined outputs avoid the reserved workflow output keys.

    Parameters
    ----------
    outputs : dict[str, Path]
        Mapping of configured GitHub Action output keys to staged artefact
        destinations.

    Raises
    ------
    StageError
        Raised when a user-defined output key overlaps with reserved keys.

    Examples
    --------
    >>> reserved = {"artifact_dir": Path("/tmp/stage/artifact")}
    >>> _validate_no_reserved_key_collisions(reserved)
    Traceback (most recent call last):
    StageError: Artefact outputs collide with reserved keys: artifact_dir
    """

    collisions = sorted(outputs.keys() & RESERVED_OUTPUT_KEYS)
    if collisions:
        message = (
            "Artefact outputs collide with reserved keys: "
            f"{collisions}"
        )
        raise StageError(message)


def stage_artefacts(config: StagingConfig, github_output_file: Path) -> StageResult:
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

    _initialize_staging_dir(staging_dir)

    staged_paths: list[Path] = []
    outputs: dict[str, Path] = {}
    checksums: dict[str, str] = {}

    for artefact in config.artefacts:
        source_path, attempts = _resolve_artefact_source(
            config.workspace, artefact, context
        )
        if source_path is None:
            if artefact.required:
                attempt_lines = ", ".join(
                    f"{attempt.template!r} -> {attempt.rendered!r}"
                    for attempt in attempts
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
            continue

        artefact_context = context | {
            "source_path": source_path.as_posix(),
            "source_name": source_path.name,
        }
        destination_text = (
            _render_template(destination, artefact_context)
            if (destination := artefact.destination)
            else source_path.name
        )

        destination_path = _safe_destination_path(staging_dir, destination_text)
        if destination_path.exists():
            destination_path.unlink()
        shutil.copy2(source_path, destination_path)
        print(
            f"Staged '{source_path.relative_to(config.workspace)}' ->"
            f" '{destination_path.relative_to(config.workspace)}'",
        )

        staged_paths.append(destination_path)
        digest = _write_checksum(destination_path, config.checksum_algorithm)
        checksums[destination_path.name] = digest

        if artefact.output:
            outputs[artefact.output] = destination_path

    if not staged_paths:
        message = "No artefacts were staged."
        raise StageError(message)

    _validate_no_reserved_key_collisions(outputs)
    exported_outputs = _prepare_output_data(
        staging_dir, staged_paths, outputs, checksums
    )
    write_github_output(github_output_file, exported_outputs)

    return StageResult(staging_dir, staged_paths, outputs, checksums)


def _render_template(template: str, context: dict[str, typ.Any]) -> str:
    """Format ``template`` with ``context`` (e.g., ``_render_template('{name}', {'name': 'bob'})`` -> ``'bob'``)."""
    try:
        return template.format(**context)
    except KeyError as exc:
        message = f"Invalid template key {exc} in '{template}'"
        raise StageError(message) from exc


def _resolve_artefact_source(
    workspace: Path, artefact: ArtefactConfig, context: dict[str, typ.Any]
) -> tuple[Path | None, list[_RenderAttempt]]:
    """Return the first artefact match and attempted renders (e.g., ``_resolve_artefact_source(Path('.'), ArtefactConfig(source='a'), {})`` -> ``(Path('a'), attempts)``)."""
    attempts: list[_RenderAttempt] = []
    patterns = [artefact.source, *artefact.alternatives]
    for pattern in patterns:
        rendered = _render_template(pattern, context)
        attempts.append(_RenderAttempt(pattern, rendered))
        if (candidate := _match_candidate_path(workspace, rendered)) is not None:
            return candidate, attempts
    return None, attempts


def _match_candidate_path(workspace: Path, rendered: str) -> Path | None:
    """Return the newest path matching ``rendered`` (e.g., ``_match_candidate_path(Path('.'), 'dist/*.zip')`` -> ``Path('dist/app')``)."""
    candidate = Path(rendered)
    base = candidate if candidate.is_absolute() else workspace / candidate
    if any(ch in rendered for ch in "*?[]"):
        if candidate.is_absolute():
            root_text, _pattern = _glob_root_and_pattern(candidate)
            root = Path(root_text)
            relative_pattern = candidate.relative_to(root).as_posix()
            candidates = [path for path in root.glob(relative_pattern) if path.is_file()]
        else:
            windows_candidate = PureWindowsPath(rendered)
            if windows_candidate.is_absolute():
                # Windows requires globbing relative to the drive root. Passing an
                # absolute pattern string such as ``C:\foo\*.txt`` causes
                # ``Path.glob`` to reject the drive prefix, so we normalise the
                # pattern to search from the drive anchor explicitly.
                anchor = Path(windows_candidate.anchor)
                relative = PureWindowsPath(*windows_candidate.parts[1:]).as_posix()
                candidates = [
                    path for path in anchor.glob(relative) if path.is_file()
                ]
            else:
                candidates = [
                    path for path in workspace.glob(rendered) if path.is_file()
                ]
        return None if not candidates else max(candidates, key=_mtime_key)
    return base if base.is_file() else None


def _mtime_key(path: Path) -> tuple[int, str]:
    """Provide a sortable key using mtime with a stable tie-breaker (e.g., ``_mtime_key(Path('file'))`` -> ``(123, 'file')``)."""
    try:
        return (int(path.stat().st_mtime_ns), path.as_posix())
    except OSError:
        return (0, path.as_posix())


def _safe_destination_path(staging_dir: Path, destination: str) -> Path:
    """Resolve ``destination`` under ``staging_dir`` (e.g., ``_safe_destination_path(Path('/tmp'), 'bin')`` -> ``Path('/tmp/bin')``)."""
    target = (staging_dir / destination).resolve()
    staging_root = staging_dir.resolve()
    if not target.is_relative_to(staging_root):
        message = f"Destination escapes staging directory: {destination}"
        raise StageError(message)
    target.parent.mkdir(parents=True, exist_ok=True)
    return target


def _write_checksum(path: Path, algorithm: str) -> str:
    """Write the checksum sidecar for ``path`` (e.g., ``_write_checksum(Path('bin'), 'sha256')`` -> ``'abc123'``)."""
    hasher = hashlib.new(algorithm)
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(8192), b""):
            hasher.update(chunk)
    digest = hasher.hexdigest()
    checksum_path = path.with_name(f"{path.name}.{algorithm}")
    checksum_path.write_text(f"{digest}  {path.name}\n", encoding="utf-8")
    return digest


def write_github_output(file: Path, values: dict[str, str | list[str]]) -> None:
    """Append ``values`` to the GitHub Actions output ``file``.

    Parameters
    ----------
    file : Path
        Target ``GITHUB_OUTPUT`` file that receives the exported values.
    values : dict[str, str | list[str]]
        Mapping of output names to values ready for GitHub Actions
        consumption.

    Examples
    --------
    >>> github_output = Path("/tmp/github_output")
    >>> write_github_output(github_output, {"name": "value"})
    >>> "name=value" in github_output.read_text()
    True
    """

    file.parent.mkdir(parents=True, exist_ok=True)
    with file.open("a", encoding="utf-8") as handle:
        for key, value in values.items():
            if isinstance(value, list):
                delimiter = f"gh_{key.upper()}"
                handle.write(f"{key}<<{delimiter}\n")
                handle.write("\n".join(value))
                handle.write(f"\n{delimiter}\n")
            else:
                escaped = (
                    value.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
                )
                handle.write(f"{key}={escaped}\n")


def _glob_root_and_pattern(candidate: PurePath) -> tuple[str, str]:
    """Return the filesystem root and relative glob pattern for ``candidate``.

    Windows globbing treats drive letters as part of the anchor. The relative
    pattern therefore needs to drop the ``C:\\`` prefix before invoking
    :meth:`Path.glob`. ``pathlib`` exposes the anchor and path parts so we can
    slice off the leading entry regardless of the host platform.
    """

    anchor = candidate.anchor
    if not anchor:
        message = f"Expected absolute path, received '{candidate}'"
        raise ValueError(message)

    root_text = (candidate.drive + candidate.root) or anchor or "/"
    relative_parts = candidate.parts[1:]
    pattern = (
        PurePosixPath(*relative_parts).as_posix() if relative_parts else "*"
    )
    return root_text, pattern
