"""Shared helpers for the staging test suites."""

from __future__ import annotations

from pathlib import Path

__all__ = ["decode_output_file", "write_workspace_inputs"]


def decode_output_file(path: Path) -> dict[str, str]:
    """Parse the key-value pairs written to ``GITHUB_OUTPUT``."""
    path = Path(path)
    lines = [line for line in path.read_text(encoding="utf-8").splitlines() if line]
    values: dict[str, str] = {}
    for line in lines:
        key, value = line.split("=", 1)
        decoded = value.replace("%0A", "\n").replace("%0D", "\r").replace("%25", "%")
        values[key] = decoded
    return values


def write_workspace_inputs(root: Path, target: str) -> None:
    """Populate ``root`` with staged artefacts for the provided ``target``."""
    root = Path(root)
    bin_path = root / "target" / target / "release" / "netsuke"
    bin_path.parent.mkdir(parents=True, exist_ok=True)
    bin_path.write_bytes(b"binary")

    man_path = root / "target" / "generated-man" / target / "release" / "netsuke.1"
    man_path.parent.mkdir(parents=True, exist_ok=True)
    man_path.write_text(".TH NETSUKE 1", encoding="utf-8")

    licence = root / "LICENSE"
    licence.write_text("Copyright Netsuke", encoding="utf-8")
