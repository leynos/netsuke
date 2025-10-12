"""Utilities for preparing and writing staging workflow outputs."""

from __future__ import annotations

import json
from pathlib import Path

from ..errors import StageError

__all__ = [
    "RESERVED_OUTPUT_KEYS",
    "_prepare_output_data",
    "_validate_no_reserved_key_collisions",
    "write_github_output",
]


RESERVED_OUTPUT_KEYS: set[str] = {
    "artifact_dir",
    "dist_dir",
    "staged_files",
    "artefact_map",
    "checksum_map",
}


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

    if collisions := sorted(outputs.keys() & RESERVED_OUTPUT_KEYS):
        message = (
            "Artefact outputs collide with reserved keys: "
            f"{collisions}"
        )
        raise StageError(message)


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
                    value.replace("%", "%25")
                    .replace("\r", "%0D")
                    .replace("\n", "%0A")
                )
                handle.write(f"{key}={escaped}\n")

