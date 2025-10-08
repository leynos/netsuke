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
    license_path : Path
        File system path to the bundled licence text.
    """

    artifact_dir: Path
    binary_path: Path
    man_path: Path
    license_path: Path


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

    bin_src, man_src, licence_src = _validate_and_locate_sources(
        workspace, config.target, config.bin_name, config.bin_ext
    )

    artifact_dir = _prepare_artifact_directory(dist_dir, config.artifact_dir_name)

    bin_dest = artifact_dir / bin_src.name
    man_dest = artifact_dir / man_src.name
    licence_dest = artifact_dir / licence_src.name
    shutil.copy2(bin_src, bin_dest)
    shutil.copy2(man_src, man_dest)
    shutil.copy2(licence_src, licence_dest)

    for path in (bin_dest, man_dest, licence_dest):
        _write_checksum(path)

    stage_result = StageResult(artifact_dir, bin_dest, man_dest, licence_dest)
    _write_github_outputs(github_output, stage_result)

    return stage_result


def _validate_and_locate_sources(
    workspace: Path, target: str, bin_name: str, bin_ext: str
) -> tuple[Path, Path, Path]:
    """Resolve the source artefacts and ensure they exist."""

    bin_src = (
        workspace / "target" / target / "release" / f"{bin_name}{bin_ext}"
    )
    if not bin_src.is_file():
        message = f"Binary not found at {bin_src}"
        raise RuntimeError(message)

    man_src = _find_manpage(workspace, target, bin_name)

    licence_src = workspace / "LICENSE"
    if not licence_src.is_file():
        message = f"Licence file not found at {licence_src}"
        raise RuntimeError(message)

    return bin_src, man_src, licence_src


def _prepare_artifact_directory(dist_dir: Path, artifact_dir_name: str) -> Path:
    """Return a clean artefact directory within the distribution workspace."""

    dist_dir.mkdir(parents=True, exist_ok=True)

    artifact_dir = dist_dir / artifact_dir_name
    if artifact_dir.exists():
        # Previous runs may leave artefacts behind; start from a clean slate so
        # releases never mix binaries or manuals from different builds.
        shutil.rmtree(artifact_dir)
    artifact_dir.mkdir(parents=True)

    return artifact_dir


def _write_github_outputs(github_output: Path, stage_result: StageResult) -> None:
    """Emit the staged artefact metadata for downstream workflow steps."""

    outputs = {
        "artifact_dir": stage_result.artifact_dir,
        "binary_path": stage_result.binary_path,
        "man_path": stage_result.man_path,
        "license_path": stage_result.license_path,
    }
    output_lines: list[str] = []
    for key, path in outputs.items():
        value = _escape_output_value(path)
        if not value:
            message = f"Resolved {key} output is unexpectedly empty"
            raise RuntimeError(message)
        output_lines.append(f"{key}={value}\n")

    with github_output.open("w", encoding="utf-8") as handle:
        handle.writelines(output_lines)


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
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    digest = h.hexdigest()
    checksum_path = Path(f"{path}.sha256")
    checksum_path.write_text(f"{digest}  {path.name}\n", encoding="utf-8")


def _escape_output_value(value: Path | str) -> str:
    """Escape workflow output values per GitHub recommendations."""
    text = value.as_posix() if isinstance(value, Path) else str(value)
    return text.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
