"""Configuration models and loader for the staging helper.

This module provides dataclasses and a loader function for parsing TOML staging
configurations that describe artefact sources, target platforms, and staging
directory templates.

Usage
-----
Load a staging configuration for a specific target::

    from pathlib import Path
    from stage_common.config import load_config

    config = load_config(Path(".github/release-staging.toml"), "windows-x86_64")
    print(f"Staging directory: {config.staging_dir()}")
"""

from __future__ import annotations

import dataclasses
import hashlib
import typing as typ
from pathlib import Path

import tomllib

from .environment import require_env_path
from .errors import StageError

__all__ = [
    "ArtefactConfig",
    "StagingConfig",
    "load_config",
]


@dataclasses.dataclass(slots=True)
class ArtefactConfig:
    """Describe a single artefact to be staged.

    Parameters
    ----------
    source : str
        ``str.format`` template pointing to the primary artefact location.
    required : bool, default=True
        When ``True`` the staging run fails if the artefact is missing.
    output : str | None, optional
        Name exported to ``GITHUB_OUTPUT`` for downstream workflow steps.
    destination : str | None, optional
        Optional ``str.format`` template for the staged filename.
    alternatives : list[str], optional
        Fallback glob patterns probed when :attr:`source` is absent.

    Attributes
    ----------
    source : str
        Raw template used to discover the artefact in the workspace.
    required : bool
        Indicates whether missing artefacts should abort staging.
    output : str | None
        Output key recorded in ``StageResult.outputs`` when provided.
    destination : str | None
        Resolved filename relative to the staging directory.
    alternatives : list[str]
        Additional glob templates evaluated in order of recency.

    Examples
    --------
    >>> cfg = ArtefactConfig(  # doctest: +SKIP
    ...     source="target/{target}/release/{bin_name}{bin_ext}",
    ...     output="binary_path",
    ... )
    >>> cfg.output  # doctest: +SKIP
    'binary_path'
    """

    source: str
    required: bool = True
    output: str | None = None
    destination: str | None = None
    alternatives: list[str] = dataclasses.field(default_factory=list)


@dataclasses.dataclass(slots=True)
class StagingConfig:
    """Concrete configuration produced by :func:`load_config`.

    Parameters
    ----------
    workspace : Path
        Repository root checked out by the GitHub Actions runner.
    bin_name : str
        Base executable name shared across targets.
    dist_dir : str
        Directory beneath :attr:`workspace` containing staged artefacts.
    checksum_algorithm : str
        Hashing algorithm used for checksum sidecars.
    artefacts : list[ArtefactConfig]
        Collection of artefacts that should be copied into staging.
    platform : str
        Human readable platform identifier (e.g. ``"linux"``).
    arch : str
        Architecture identifier appended to the staging directory name.
    target : str
        Full compilation target triple.
    bin_ext : str, default=""
        Optional suffix appended to the staged executable name.
    staging_dir_template : str, default="{bin_name}_{platform}_{arch}"
        ``str.format`` template used to build :attr:`staging_dir_name`.
    target_key : str | None, optional
        Name of the TOML ``[targets.*]`` entry used to build this config.

    Attributes
    ----------
    workspace : Path
        The root workspace directory as a :class:`pathlib.Path`.
    bin_name : str
        Executable base name reused across targets.
    dist_dir : str
        Directory containing staged release artefacts.
    checksum_algorithm : str
        Normalised checksum algorithm name.
    artefacts : list[ArtefactConfig]
        Ordered artefact definitions that will be staged.
    platform : str
        Platform identifier used for logging and templating.
    arch : str
        Architecture identifier used for templating.
    target : str
        Compilation target triple.
    bin_ext : str
        Optional executable extension such as ``".exe"``.
    staging_dir_template : str
        Template used to derive :attr:`staging_dir_name`.
    target_key : str | None
        Name of the target section consumed from the TOML file.

    Examples
    --------
    >>> from pathlib import Path  # doctest: +SKIP
    >>> config = StagingConfig(  # doctest: +SKIP
    ...     workspace=Path("/tmp/workspace"),
    ...     bin_name="netsuke",
    ...     dist_dir="dist",
    ...     checksum_algorithm="sha256",
    ...     artefacts=[ArtefactConfig(source="LICENSE")],
    ...     platform="linux",
    ...     arch="amd64",
    ...     target="x86_64-unknown-linux-gnu",
    ... )
    >>> config.staging_dir_name  # doctest: +SKIP
    'netsuke_linux_amd64'
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
        """Directory name rendered from :attr:`staging_dir_template`."""
        return self.as_template_context()["staging_dir_name"]

    def as_template_context(self) -> dict[str, typ.Any]:
        """Return a mapping suitable for rendering ``str.format`` templates."""
        base_context: dict[str, typ.Any] = {
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
        template_context = base_context | {
            "staging_dir_template": self.staging_dir_template
        }
        rendered_name = self.staging_dir_template.format(**template_context)
        return template_context | {"staging_dir_name": rendered_name}


def load_config(config_file: Path, target_key: str) -> StagingConfig:
    """Load staging configuration from ``config_file`` for ``target_key``.

    Parameters
    ----------
    config_file : Path
        Path to the TOML configuration file describing staging inputs.
    target_key : str
        Key identifying the target-specific configuration section to load.

    Returns
    -------
    StagingConfig
        Fully realised configuration containing resolved paths and artefacts.

    Raises
    ------
    FileNotFoundError
        Raised when the configuration file is absent at ``config_file``.
    StageError
        Raised when required configuration keys are missing or invalid.
    """
    config_file = Path(config_file)
    if not config_file.is_file():
        message = f"Configuration file not found at {config_file}"
        raise FileNotFoundError(message)

    data = _load_toml(config_file)
    common, target_cfg = _extract_sections(data, config_file, target_key)
    _require_keys(common, {"bin_name"}, "common", config_file)
    _require_keys(
        target_cfg,
        {"platform", "arch", "target"},
        f"targets.{target_key}",
        config_file,
    )
    workspace = require_env_path("GITHUB_WORKSPACE")
    algorithm = _validate_checksum(common.get("checksum_algorithm"))
    artefacts = _make_artefacts(common, target_cfg, config_file)

    return StagingConfig(
        workspace=workspace,
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


def _load_toml(path: Path) -> dict[str, typ.Any]:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def _extract_sections(
    data: dict[str, typ.Any], config_path: Path, target_key: str
) -> tuple[dict[str, typ.Any], dict[str, typ.Any]]:
    try:
        common = data["common"]
        target_cfg = data["targets"][target_key]
    except KeyError as exc:
        message = f"Missing configuration key in {config_path}: {exc}"
        raise StageError(message) from exc
    return common, target_cfg


def _validate_checksum(name: str | None) -> str:
    algorithm = (name or "sha256").lower()
    supported = {item.lower() for item in hashlib.algorithms_guaranteed}
    if algorithm not in supported:
        message = f"Unsupported checksum algorithm: {algorithm}"
        raise StageError(message)
    try:
        hashlib.new(algorithm)
    except ValueError as exc:
        message = (
            f"Checksum algorithm not supported by hashlib.new: {algorithm}"
        )
        raise StageError(message) from exc
    return algorithm


def _make_artefacts(
    common: dict[str, typ.Any],
    target_cfg: dict[str, typ.Any],
    config_path: Path,
) -> list[ArtefactConfig]:
    entries = [*common.get("artefacts", []), *target_cfg.get("artefacts", [])]
    if not entries:
        message = "No artefacts configured to stage."
        raise StageError(message)
    artefacts: list[ArtefactConfig] = []
    for index, entry in enumerate(entries, start=1):
        if not isinstance(entry, dict):
            message = (
                "Artefact entries must be tables of key/value pairs "
                f"(entry #{index} in {config_path})"
            )
            raise StageError(message)
        source = entry.get("source")
        if not isinstance(source, str) or not source:
            message = (
                "Missing required artefact key 'source' "
                f"in entry #{index} of {config_path}"
            )
            raise StageError(message)
        alternatives = _normalise_alternatives(
            entry.get("alternatives", []), index, config_path
        )
        artefacts.append(
            ArtefactConfig(
                source=source,
                required=entry.get("required", True),
                output=entry.get("output"),
                destination=entry.get("destination"),
                alternatives=alternatives,
            )
        )
    return artefacts


def _require_keys(
    section: dict[str, typ.Any], keys: set[str], label: str, config_path: Path
) -> None:
    """Ensure ``section`` defines ``keys``.

    Examples
    --------
    >>> _require_keys(  # doctest: +SKIP
    ...     {'bin': 1},
    ...     {'bin'},
    ...     'common',
    ...     Path('cfg'),
    ... )
    """
    if missing := sorted(key for key in keys if key not in section):
        joined = ", ".join(missing)
        message = (
            "Missing required key(s) "
            f"{joined} in [{label}] section of {config_path}"
        )
        raise StageError(message)


def _normalise_alternatives(
    value: object, index: int, config_path: Path
) -> list[str]:
    """Return ``value`` as a list of alternative glob patterns."""

    if value is None:
        return []
    if isinstance(value, str):
        return [value] if value else []
    if not isinstance(value, list):
        message = (
            "Alternatives must be a list of strings "
            f"(entry #{index} in {config_path})"
        )
        raise StageError(message)
    alternatives: list[str] = []
    for alternative in value:
        if not isinstance(alternative, str):
            message = (
                "Alternatives must be strings "
                f"(entry #{index} in {config_path})"
            )
            raise StageError(message)
        if alternative:
            alternatives.append(alternative)
    return alternatives
