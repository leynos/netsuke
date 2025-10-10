"""Artefact staging logic shared across the CLI and composite action."""

from __future__ import annotations

import dataclasses
import hashlib
import json
import shutil
import sys
import typing as typ
from pathlib import Path, PureWindowsPath

from .errors import StageError

if typ.TYPE_CHECKING:
    from .config import ArtefactConfig, StagingConfig

__all__ = ["StageResult", "stage_artefacts"]


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

    if staging_dir.exists():
        shutil.rmtree(staging_dir)
    staging_dir.mkdir(parents=True, exist_ok=True)

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

    staged_files_value = "\n".join(path.name for path in sorted(staged_paths))
    artefact_map_json = json.dumps(
        {key: path.as_posix() for key, path in sorted(outputs.items())}
    )
    checksum_map_json = json.dumps(dict(sorted(checksums.items())))

    exported_outputs: dict[str, str | list[str]] = {
        "artifact_dir": staging_dir.as_posix(),
        "dist_dir": staging_dir.parent.as_posix(),
        "staged_files": staged_files_value,
        "artefact_map": artefact_map_json,
        "checksum_map": checksum_map_json,
    } | {key: path.as_posix() for key, path in outputs.items()}

    _write_to_github_output(github_output_file, exported_outputs)

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
            root = Path(candidate.anchor or "/")
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


def _write_to_github_output(file: Path, values: dict[str, str | list[str]]) -> None:
    """Append outputs to ``file`` using GitHub's format (e.g., ``_write_to_github_output(Path('out'), {'k': 'v'})`` -> ``None``)."""
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
