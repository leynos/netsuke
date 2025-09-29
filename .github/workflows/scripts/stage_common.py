# /// script
# requires-python = ">=3.11"
# ///

"""Shared helpers for staging release artefacts."""

from __future__ import annotations

import dataclasses
import hashlib
import shutil
import typing as typ
from pathlib import Path

__all__ = ["StageResult", "StagingConfig", "stage_artifacts"]


@dataclasses.dataclass(frozen=True)
class StagingConfig:
    """Immutable bundle describing a staged binary."""

    bin_name: str
    target: str
    platform: str
    arch: str
    workspace: Path
    bin_ext: str = ""

    @property
    def artifact_dir_name(self) -> str:
        """Return the directory name used for the staged artefacts."""
        return f"{self.bin_name}_{self.platform}_{self.arch}"


class StageResult(typ.NamedTuple):
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


def stage_artifacts(config: StagingConfig, github_output: Path) -> StageResult:
    """Copy artefacts, emit checksums, and record metadata.

    Parameters
    ----------
    config : StagingConfig
        Aggregated configuration describing the binary, build target, and
        staging workspace. ``config.bin_ext`` may append platform-specific
        suffixes such as ``".exe"`` when locating binaries.
    github_output : Path
        File that receives workflow output key-value pairs.

    Returns
    -------
    StageResult
        Paths for the staged artefact directory, binary, and manual page.

    Raises
    ------
    RuntimeError
        If the binary or manual page cannot be located uniquely.
    """
    workspace = config.workspace.resolve()
    dist_dir = workspace / "dist"
    dist_dir.mkdir(parents=True, exist_ok=True)

    bin_src = (
        workspace
        / "target"
        / config.target
        / "release"
        / f"{config.bin_name}{config.bin_ext}"
    )
    if not bin_src.is_file():
        message = f"Binary not found at {bin_src}"
        raise RuntimeError(message)

    man_src = _find_manpage(workspace, config.target, config.bin_name)

    artifact_dir = dist_dir / config.artifact_dir_name
    if artifact_dir.exists():
        shutil.rmtree(artifact_dir)
    artifact_dir.mkdir(parents=True)

    bin_dest = artifact_dir / bin_src.name
    man_dest = artifact_dir / man_src.name
    shutil.copy2(bin_src, bin_dest)
    shutil.copy2(man_src, man_dest)

    for path in (bin_dest, man_dest):
        _write_checksum(path)

    with github_output.open("a", encoding="utf-8") as handle:
        handle.write(f"artifact_dir={_escape_output_value(artifact_dir)}\n")
        handle.write(f"binary_path={_escape_output_value(bin_dest)}\n")
        handle.write(f"man_path={_escape_output_value(man_dest)}\n")

    return StageResult(artifact_dir, bin_dest, man_dest)


def _find_manpage(workspace: Path, target: str, bin_name: str) -> Path:
    """Locate exactly one man page.

    Precedence:

    1) ``target/generated-man/<target>/release/{bin}.1`` (preferred).
    2) ``target/<target>/release/build/*/out/{bin}.1``; when several matches
       exist, prefer the newest by modification time and fall back to
       lexicographic ordering for ties.

    Raises
    ------
    RuntimeError
        If no candidate exists for ``target``.
    """
    generated = (
        workspace / "target" / "generated-man" / target / "release" / f"{bin_name}.1"
    )
    if generated.is_file():
        return generated

    build_root = workspace / "target" / target / "release" / "build"
    if build_root.is_dir():
        matches = [
            candidate
            for candidate in build_root.glob(f"*/out/{bin_name}.1")
            if candidate.is_file()
        ]
        if matches:
            if len(matches) == 1:
                return matches[0]

            def _sort_key(path: Path) -> tuple[int, str]:
                try:
                    return (int(path.stat().st_mtime_ns), path.as_posix())
                except OSError:
                    return (0, path.as_posix())

            matches.sort(key=_sort_key)
            return matches[-1]

    message = f"Man page not found for target {target}"
    raise RuntimeError(message)


def _write_checksum(path: Path) -> None:
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    checksum_path = Path(f"{path}.sha256")
    checksum_path.write_text(f"{digest}  {path.name}\n", encoding="utf-8")


def _escape_output_value(value: Path | str) -> str:
    """Escape workflow output values per GitHub recommendations."""
    text = value.as_posix() if isinstance(value, Path) else str(value)
    return text.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
