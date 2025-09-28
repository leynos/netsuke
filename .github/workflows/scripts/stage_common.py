# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "cyclopts>=3.24.0,<4.0.0",
# ]
# ///

"""Shared helpers for staging release artefacts."""

from __future__ import annotations

import dataclasses
import hashlib
import shutil
import typing as typ
from pathlib import Path


@dataclasses.dataclass(frozen=True)
class BinaryConfig:
    """Configuration describing the staged binary."""

    name: str
    extension: str = ""


@dataclasses.dataclass(frozen=True)
class BuildConfig:
    """Compilation metadata for locating build outputs."""

    target: str
    platform: str
    arch: str


@dataclasses.dataclass(frozen=True)
class PathConfig:
    """Filesystem locations used during staging."""

    workspace: Path
    github_output: Path


@dataclasses.dataclass(frozen=True)
class StageConfig:
    """Aggregate configuration required to stage release artefacts."""

    bin_name: str
    target: str
    platform: str
    arch: str
    workspace: Path
    github_output: Path
    bin_ext: str = ""


@dataclasses.dataclass(frozen=True)
class StageResult:
    """Immutable summary of staged artefacts.

    Attributes
    ----------
    artifact_dir : Path
        Directory containing the staged artefact bundle.
    binary_path : Path
        File system path to the staged binary.
    man_path : Path
        File system path to the staged manual page.
    """

    artifact_dir: Path
    binary_path: Path
    man_path: Path


def stage_artifacts(*, config: StageConfig) -> StageResult:
    """Copy artefacts, emit checksums, and record metadata.

    Parameters
    ----------
    config : StageConfig
        Aggregated configuration describing the binary, build target, and
        required filesystem locations. ``config.bin_ext`` is optional and may
        be used to append platform-specific suffixes such as ``".exe"``.

    Returns
    -------
    StageResult
        Paths for the staged artefact directory, binary, and manual page.

    Raises
    ------
    RuntimeError
        If the binary or manual page cannot be located uniquely.
    """
    binary_config = BinaryConfig(config.bin_name, config.bin_ext)
    build_config = BuildConfig(config.target, config.platform, config.arch)
    path_config = PathConfig(config.workspace, config.github_output)

    workspace = path_config.workspace.resolve()
    dist_dir = workspace / "dist"
    dist_dir.mkdir(parents=True, exist_ok=True)

    bin_src = (
        workspace
        / "target"
        / build_config.target
        / "release"
        / f"{binary_config.name}{binary_config.extension}"
    )
    if not bin_src.is_file():
        message = f"Binary not found at {bin_src}"
        raise RuntimeError(message)

    man_candidates = _collect_manpage_candidates(
        workspace=workspace,
        target=build_config.target,
        bin_name=binary_config.name,
    )
    man_src = _select_single_candidate(man_candidates, build_config.target)

    artifact_dir = (
        dist_dir / f"{binary_config.name}_{build_config.platform}_{build_config.arch}"
    )
    artifact_dir.mkdir(parents=True, exist_ok=True)

    bin_dest = artifact_dir / bin_src.name
    man_dest = artifact_dir / man_src.name
    shutil.copy2(bin_src, bin_dest)
    shutil.copy2(man_src, man_dest)

    for path in (bin_dest, man_dest):
        _write_checksum(path)

    with path_config.github_output.open("a", encoding="utf-8") as handle:
        handle.write(f"artifact_dir={artifact_dir.as_posix()}\n")
        handle.write(f"binary_path={bin_dest.as_posix()}\n")
        handle.write(f"man_path={man_dest.as_posix()}\n")

    return StageResult(artifact_dir, bin_dest, man_dest)


def _collect_manpage_candidates(
    *, workspace: Path, target: str, bin_name: str
) -> list[Path]:
    candidates: list[Path] = []

    generated = (
        workspace / "target" / "generated-man" / target / "release" / f"{bin_name}.1"
    )
    if generated.is_file():
        candidates.append(generated)

    build_root = workspace / "target" / target / "release" / "build"
    if build_root.is_dir():
        matches = [
            candidate
            for candidate in build_root.glob("*/out/*.1")
            if candidate.name == f"{bin_name}.1"
        ]
        candidates.extend(matches)

    return list(dict.fromkeys(candidates))


def _select_single_candidate(candidates: typ.Iterable[Path], target: str) -> Path:
    unique = list(candidates)
    if not unique:
        message = f"Man page not found for target {target}"
        raise RuntimeError(message)
    if len(unique) > 1:
        locations = "\n".join(str(path) for path in unique)
        message = "Multiple man page candidates found:\n" + locations
        raise RuntimeError(message)
    return unique[0]


def _write_checksum(path: Path) -> None:
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    checksum_path = Path(f"{path}.sha256")
    checksum_path.write_text(f"{digest}  {path.name}\n", encoding="utf-8")
