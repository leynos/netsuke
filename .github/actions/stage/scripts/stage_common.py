"""Reusable helpers for staging release artefacts.

The helpers abstract the mechanics of loading TOML configuration files,
preparing staging directories, copying artefacts, and emitting GitHub
Actions outputs. They are designed for reuse across projects so long as the
consumer provides a configuration file that follows the schema described in
``load_config``.
"""

from __future__ import annotations

import dataclasses
import hashlib
import json
import os
import shutil
import sys
import typing as typ
from pathlib import Path

import tomllib

__all__ = [
    "ArtefactConfig",
    "StageError",
    "StageResult",
    "StagingConfig",
    "load_config",
    "stage_artefacts",
]


class StageError(RuntimeError):
    """Raised when the staging pipeline cannot continue."""


@dataclasses.dataclass(slots=True)
class ArtefactConfig:
    """Describe a single artefact to be staged.

    Parameters
    ----------
    source:
        Template rendered with the staging context to locate the artefact on
        disk. Templates use ``str.format``.
    required:
        If ``True`` the staging pipeline aborts when the artefact is missing.
        Optional artefacts emit a GitHub Actions warning instead.
    output:
        Optional name for the artefact in the exported outputs mapping. Use
        this to reference critical files (for example ``"binary_path"``).
    destination:
        Optional template describing the destination filename within the
        staging directory. Defaults to the source filename.
    alternatives:
        Additional templates or glob patterns used when ``source`` does not
        resolve to a file. The first existing match wins.

    Examples
    --------
    >>> cfg = ArtefactConfig(source="target/{target}/release/{bin_name}")
    >>> cfg.source
    'target/{target}/release/{bin_name}'
    """

    source: str
    required: bool = True
    output: str | None = None
    destination: str | None = None
    alternatives: list[str] = dataclasses.field(default_factory=list)


@dataclasses.dataclass(slots=True)
class StagingConfig:
    """Concrete configuration produced by :func:`load_config`.

    The configuration merges ``[common]`` values with target-specific
    overrides. Template rendering uses the dictionary returned by
    :meth:`as_template_context`.
    """

    workspace: Path
    bin_name: str
    dist_dir: str
    checksum_algorithm: str
    artefacts: list[ArtefactConfig]
    platform: str
    arch: str
    target: str
    bin_ext: str = ""
    staging_dir_template: str = "{bin_name}_{platform}_{arch}"
    target_key: str | None = None

    def staging_dir(self) -> Path:
        """Return the absolute staging directory path."""
        return self.workspace / self.dist_dir / self.staging_dir_name

    @property
    def staging_dir_name(self) -> str:
        """Directory name rendered from :attr:`staging_dir_template`.

        Examples
        --------
        >>> config = StagingConfig(
        ...     workspace=Path("/tmp/workspace"),
        ...     bin_name="netsuke",
        ...     dist_dir="dist",
        ...     checksum_algorithm="sha256",
        ...     artefacts=[],
        ...     platform="linux",
        ...     arch="amd64",
        ...     target="x86_64-unknown-linux-gnu",
        ... )
        >>> config.staging_dir_name
        'netsuke_linux_amd64'
        """
        context = self._base_template_context()
        context["staging_dir_name"] = self.staging_dir_template
        context["staging_dir_template"] = self.staging_dir_template
        return self.staging_dir_template.format(**context)

    def as_template_context(self) -> dict[str, typ.Any]:
        """Return a mapping for template rendering."""
        context = self._base_template_context()
        context["staging_dir_template"] = self.staging_dir_template
        context["staging_dir_name"] = self.staging_dir_name
        return context

    def _base_template_context(self) -> dict[str, typ.Any]:
        return {
            "workspace": self.workspace.as_posix(),
            "bin_name": self.bin_name,
            "dist_dir": self.dist_dir,
            "checksum_algorithm": self.checksum_algorithm,
            "platform": self.platform,
            "arch": self.arch,
            "target": self.target,
            "bin_ext": self.bin_ext or "",
            "target_key": self.target_key or "",
        }


@dataclasses.dataclass(slots=True)
class StageResult:
    """Outcome of :func:`stage_artefacts`.

    Attributes
    ----------
    staging_dir:
        Directory containing the staged artefacts.
    staged_artefacts:
        List of paths for artefacts copied into ``staging_dir``.
    outputs:
        Mapping of logical names (``ArtefactConfig.output``) to staged paths.
    checksums:
        Mapping of staged filenames to checksum digests.

    Examples
    --------
    >>> result = StageResult(Path("dist"), [], {}, {})
    >>> result.staging_dir
    PosixPath('dist')
    """

    staging_dir: Path
    staged_artefacts: list[Path]
    outputs: dict[str, Path]
    checksums: dict[str, str]


def load_config(config_file: Path, target_key: str) -> StagingConfig:
    """Load staging configuration from ``config_file``.

    Parameters
    ----------
    config_file:
        Path to the TOML document describing the staging configuration.
    target_key:
        Key in the ``[targets]`` table selecting platform-specific overrides.

    Returns
    -------
    StagingConfig
        Fully merged configuration ready for use by :func:`stage_artefacts`.

    Raises
    ------
    FileNotFoundError
        If ``config_file`` does not exist.
    StageError
        When the configuration is malformed or mandatory settings are missing.
    """
    if not config_file.is_file():
        message = f"Configuration file not found at {config_file}"
        raise FileNotFoundError(message)

    with config_file.open("rb") as handle:
        data = tomllib.load(handle)

    try:
        common = data["common"]
        targets = data["targets"]
        target_cfg = targets[target_key]
    except KeyError as exc:
        message = f"Missing configuration key in {config_file}: {exc}"
        raise StageError(message) from exc

    workspace_env = os.environ.get("GITHUB_WORKSPACE")
    if not workspace_env:
        message = "GITHUB_WORKSPACE environment variable is not set."
        raise StageError(message)

    algorithm = (common.get("checksum_algorithm") or "sha256").lower()
    if algorithm not in {name.lower() for name in hashlib.algorithms_available}:
        message = f"Unsupported checksum algorithm: {algorithm}"
        raise StageError(message)

    artefact_entries: list[dict[str, typ.Any]] = []
    artefact_entries.extend(common.get("artefacts", []))
    artefact_entries.extend(target_cfg.get("artefacts", []))
    artefacts = [
        ArtefactConfig(
            source=entry["source"],
            required=entry.get("required", True),
            output=entry.get("output"),
            destination=entry.get("destination"),
            alternatives=entry.get("alternatives", []),
        )
        for entry in artefact_entries
    ]

    if not artefacts:
        message = "No artefacts configured to stage."
        raise StageError(message)

    return StagingConfig(
        workspace=Path(workspace_env),
        bin_name=common["bin_name"],
        dist_dir=common.get("dist_dir", "dist"),
        checksum_algorithm=algorithm,
        artefacts=artefacts,
        platform=target_cfg["platform"],
        arch=target_cfg["arch"],
        target=target_cfg["target"],
        bin_ext=target_cfg.get("bin_ext", ""),
        staging_dir_template=target_cfg.get(
            "staging_dir_template",
            common.get("staging_dir_template", "{bin_name}_{platform}_{arch}"),
        ),
        target_key=target_key,
    )


def stage_artefacts(config: StagingConfig, github_output_file: Path) -> StageResult:
    """Copy artefacts into ``config``'s staging directory.

    Parameters
    ----------
    config:
        Configuration returned by :func:`load_config`.
    github_output_file:
        File that receives workflow outputs per the GitHub Actions protocol.

    Returns
    -------
    StageResult
        Summary of staged artefacts and exported outputs.

    Examples
    --------
    >>> tmp = Path("/tmp")  # doctest: +SKIP
    >>> cfg = StagingConfig(  # doctest: +SKIP
    ...     workspace=tmp,
    ...     bin_name="netsuke",
    ...     dist_dir="dist",
    ...     checksum_algorithm="sha256",
    ...     artefacts=[ArtefactConfig(source="LICENSE", output="license_path")],
    ...     platform="linux",
    ...     arch="amd64",
    ...     target="x86_64-unknown-linux-gnu",
    ... )
    >>> stage_artefacts(cfg, tmp / "out")  # doctest: +SKIP
    StageResult(...)
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
        source_path = _resolve_artefact_source(config.workspace, artefact, context)
        if source_path is None:
            if artefact.required:
                message = f"Required artefact not found for template: {artefact.source}"
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
        destination = artefact.destination
        if destination:
            destination_text = _render_template(destination, artefact_context)
        else:
            destination_text = source_path.name

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
    }
    exported_outputs.update({key: path.as_posix() for key, path in outputs.items()})

    _write_to_github_output(github_output_file, exported_outputs)

    return StageResult(staging_dir, staged_paths, outputs, checksums)


def _render_template(template: str, context: dict[str, typ.Any]) -> str:
    try:
        return template.format(**context)
    except KeyError as exc:
        message = f"Invalid template key {exc} in '{template}'"
        raise StageError(message) from exc


def _resolve_artefact_source(
    workspace: Path, artefact: ArtefactConfig, context: dict[str, typ.Any]
) -> Path | None:
    patterns = [artefact.source, *artefact.alternatives]
    for pattern in patterns:
        rendered = _render_template(pattern, context)
        candidate = _match_candidate_path(workspace, rendered)
        if candidate is not None:
            return candidate
    return None


def _match_candidate_path(workspace: Path, rendered: str) -> Path | None:
    base_path = Path(rendered)
    looks_like_glob = any(ch in rendered for ch in "*?[]")

    if base_path.is_absolute():
        search_pattern = rendered
    else:
        search_pattern = (workspace / rendered).as_posix()

    if looks_like_glob:
        if base_path.is_absolute():
            root = Path(base_path.anchor or "/")
            pattern = base_path.as_posix().lstrip("/")
            matches = list(root.glob(pattern))
        else:
            matches = list(workspace.glob(rendered))
        matches = [match for match in matches if match.is_file()]
        if not matches:
            return None
        matches.sort(key=_candidate_sort_key)
        return matches[-1]

    candidate = Path(search_pattern)
    return candidate if candidate.is_file() else None


def _candidate_sort_key(path: Path) -> tuple[int, str]:
    try:
        return (int(path.stat().st_mtime_ns), path.as_posix())
    except OSError:
        return (0, path.as_posix())


def _safe_destination_path(staging_dir: Path, destination: str) -> Path:
    destination_path = Path(destination)
    if destination_path.is_absolute():
        message = f"Destination must be relative: {destination}"
        raise StageError(message)
    resolved = (staging_dir / destination_path).resolve()
    try:
        resolved.relative_to(staging_dir.resolve())
    except ValueError as exc:
        message = f"Destination escapes staging directory: {destination}"
        raise StageError(message) from exc
    resolved.parent.mkdir(parents=True, exist_ok=True)
    return resolved


def _write_checksum(path: Path, algorithm: str) -> str:
    hasher = hashlib.new(algorithm)
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(8192), b""):
            hasher.update(chunk)
    digest = hasher.hexdigest()
    checksum_path = path.with_name(f"{path.name}.{algorithm}")
    checksum_path.write_text(f"{digest}  {path.name}\n", encoding="utf-8")
    return digest


def _write_to_github_output(file: Path, values: dict[str, str | list[str]]) -> None:
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
