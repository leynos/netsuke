"""Configuration models and loader for the staging helper."""

from __future__ import annotations

import dataclasses
import hashlib
import os
import typing as typ
from pathlib import Path

import tomllib

from .errors import StageError

__all__ = [
    "ArtefactConfig",
    "StagingConfig",
    "load_config",
]


@dataclasses.dataclass(slots=True)
class ArtefactConfig:
    """Describe a single artefact to be staged."""

    source: str
    required: bool = True
    output: str | None = None
    destination: str | None = None
    alternatives: list[str] = dataclasses.field(default_factory=list)


@dataclasses.dataclass(slots=True)
class StagingConfig:
    """Concrete configuration produced by :func:`load_config`."""

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
        """Return a mapping suitable for rendering str.format templates."""
        ctx = dataclasses.asdict(self)
        ctx.pop("artefacts", None)
        ctx["workspace"] = self.workspace.as_posix()
        ctx["bin_ext"] = self.bin_ext or ""
        ctx["target_key"] = self.target_key or ""
        ctx["staging_dir_template"] = self.staging_dir_template
        ctx["staging_dir_name"] = self.staging_dir_template.format(**ctx)
        return ctx


def load_config(config_file: Path, target_key: str) -> StagingConfig:
    """Load staging configuration from ``config_file`` for ``target_key``."""
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
    supported = {name.lower() for name in hashlib.algorithms_available}
    if algorithm not in supported:
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
