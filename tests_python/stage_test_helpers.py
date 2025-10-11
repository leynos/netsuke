"""Shared helpers for the staging test suites."""

from __future__ import annotations

from pathlib import Path

__all__ = ["decode_output_file", "write_workspace_inputs"]


def decode_output_file(path: Path) -> dict[str, str]:
    """Parse GitHub output records written with ``write_github_output``.

    Parameters
    ----------
    path : Path
        Path to the output file containing GitHub workflow output records.

    Returns
    -------
    dict[str, str]
        Mapping of output keys to their decoded string values.
    """

    lines = path.read_text(encoding="utf-8").splitlines()
    values: dict[str, str] = {}
    index = 0
    while index < len(lines):
        line = lines[index]
        if "<<" in line:
            key, delimiter = line.split("<<", 1)
            index += 1
            buffer: list[str] = []
            while index < len(lines) and lines[index] != delimiter:
                buffer.append(lines[index])
                index += 1
            values[key] = "\n".join(buffer)
            index += 1  # Skip the delimiter terminator.
            continue
        if "=" in line:
            key, value = line.split("=", 1)
            decoded = (
                value.replace("%0A", "\n")
                .replace("%0D", "\r")
                .replace("%25", "%")
            )
            values[key] = decoded
        index += 1
    return values


def write_workspace_inputs(root: Path, target: str) -> None:
    """Populate ``root`` with staged artefacts for the provided ``target``.

    Parameters
    ----------
    root : Path
        Workspace root directory to populate with test artefacts.
    target : str
        Target triple identifying the compilation target.
    """
    bin_path = root / "target" / target / "release" / "netsuke"
    bin_path.parent.mkdir(parents=True, exist_ok=True)
    bin_path.write_bytes(b"binary")

    man_path = root / "target" / "generated-man" / target / "release" / "netsuke.1"
    man_path.parent.mkdir(parents=True, exist_ok=True)
    man_path.write_text(".TH NETSUKE 1", encoding="utf-8")

    licence = root / "LICENSE"
    licence.write_text("Copyright Netsuke", encoding="utf-8")
