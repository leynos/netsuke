"""Core artefact staging pipeline and supporting helpers."""

from __future__ import annotations

import dataclasses
import hashlib
import shutil
import sys
import typing as typ
from pathlib import Path

from ..errors import StageError
from .output import (
    _prepare_output_data,
    _validate_no_reserved_key_collisions,
    write_github_output,
)
from .resolution import _match_candidate_path

if typ.TYPE_CHECKING:
    from ..config import ArtefactConfig, StagingConfig

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


@dataclasses.dataclass(slots=True)
class _StagingEnvironment:
    """Encapsulates the staging directory and template context."""

    staging_dir: Path
    context: dict[str, typ.Any]


@dataclasses.dataclass(slots=True, frozen=True)
class StagedArtefact:
    """Describe a staged artefact yielded by :func:`_iter_staged_artefacts`."""

    path: Path
    artefact: ArtefactConfig
    checksum: str


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

    _initialize_staging_dir(staging_dir)

    staged_paths: list[Path] = []
    outputs: dict[str, Path] = {}
    checksums: dict[str, str] = {}

    for staged in _iter_staged_artefacts(config, staging_dir, context):
        staged_paths.append(staged.path)
        checksums[staged.path.name] = staged.checksum

        if staged.artefact.output:
            outputs[staged.artefact.output] = staged.path

    if not staged_paths:
        message = "No artefacts were staged."
        raise StageError(message)

    _validate_no_reserved_key_collisions(outputs)
    exported_outputs = _prepare_output_data(
        staging_dir, staged_paths, outputs, checksums
    )
    write_github_output(github_output_file, exported_outputs)

    return StageResult(staging_dir, staged_paths, outputs, checksums)


def _ensure_source_available(
    source_path: Path | None,
    artefact: "ArtefactConfig",
    attempts: list[_RenderAttempt],
    workspace: Path,
) -> bool:
    """Return ``True`` when ``source_path`` exists, otherwise handle the miss."""

    if source_path is not None:
        return True

    if artefact.required:
        attempt_lines = ", ".join(
            f"{attempt.template!r} -> {attempt.rendered!r}" for attempt in attempts
        )
        message = (
            "Required artefact not found. "
            f"Workspace={workspace.as_posix()} "
            f"Attempts=[{attempt_lines}]"
        )
        raise StageError(message)

    warning = (
        "::warning title=Artefact Skipped::Optional artefact missing: "
        f"{artefact.source}"
    )
    print(warning, file=sys.stderr)
    return False


def _iter_staged_artefacts(
    config: "StagingConfig", staging_dir: Path, context: dict[str, typ.Any]
) -> typ.Iterator[StagedArtefact]:
    """Yield :class:`StagedArtefact` entries describing staged artefacts.

    Examples
    --------
    >>> from pathlib import Path
    >>> from types import SimpleNamespace
    >>> workspace = Path('.')
    >>> artefact = SimpleNamespace(
    ...     source='missing', alternatives=[], required=False, destination=None, output=None
    ... )
    >>> staged = list(
    ...     _iter_staged_artefacts(
    ...         SimpleNamespace(
    ...             workspace=workspace,
    ...             artefacts=[artefact],
    ...             checksum_algorithm='sha256',
    ...         ),
    ...         workspace / 'stage',
    ...         {},
    ...     )
    ... )
    >>> staged
    []
    """

    env = _StagingEnvironment(staging_dir=staging_dir, context=context)

    for artefact in config.artefacts:
        source_path, attempts = _resolve_artefact_source(
            config.workspace, artefact, context
        )
        if not _ensure_source_available(
            source_path, artefact, attempts, config.workspace
        ):
            continue

        destination_path = _stage_single_artefact(
            config, env, artefact, typ.cast(Path, source_path)
        )
        digest = _write_checksum(destination_path, config.checksum_algorithm)
        yield StagedArtefact(destination_path, artefact, digest)


def _stage_single_artefact(
    config: "StagingConfig",
    env: _StagingEnvironment,
    artefact: "ArtefactConfig",
    source_path: Path,
) -> Path:
    """Copy ``source_path`` into ``env.staging_dir`` and return the staged path."""

    artefact_context = env.context | {
        "source_path": source_path.as_posix(),
        "source_name": source_path.name,
    }
    destination_text = (
        _render_template(destination, artefact_context)
        if (destination := artefact.destination)
        else source_path.name
    )

    destination_path = _safe_destination_path(env.staging_dir, destination_text)
    if destination_path.exists():
        destination_path.unlink()
    shutil.copy2(source_path, destination_path)
    print(
        f"Staged '{source_path.relative_to(config.workspace)}' ->"
        f" '{destination_path.relative_to(config.workspace)}'",
    )
    return destination_path


def _render_template(template: str, context: dict[str, typ.Any]) -> str:
    """Format ``template`` with ``context`` (e.g., ``_render_template('{name}', {'name': 'bob'})`` -> ``'bob'``)."""

    try:
        return template.format(**context)
    except KeyError as exc:
        message = f"Invalid template key {exc} in '{template}'"
        raise StageError(message) from exc


def _resolve_artefact_source(
    workspace: Path, artefact: "ArtefactConfig", context: dict[str, typ.Any]
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

