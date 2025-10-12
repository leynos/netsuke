"""Helpers for writing GitHub Actions outputs."""

from __future__ import annotations

import uuid
from collections.abc import Mapping, Sequence
from pathlib import Path

__all__ = ["write_github_output"]


def write_github_output(
    file: Path, values: Mapping[str, str | Sequence[str]]
) -> None:
    """Append ``values`` to ``file`` using GitHub's multiline syntax.

    Parameters
    ----------
    file:
        Path to the GitHub Actions output file (typically ``GITHUB_OUTPUT``).
    values:
        Mapping of output keys to string or sequence values. Sequence values
        are joined with newlines before being written.

    Returns
    -------
    None
        This helper writes to ``file`` for its side effect.
    """

    file.parent.mkdir(parents=True, exist_ok=True)
    with file.open("a", encoding="utf-8") as handle:
        for key, value in values.items():
            delimiter = f"EOF_{uuid.uuid4().hex}"
            handle.write(f"{key}<<{delimiter}\n")
            if isinstance(value, Sequence) and not isinstance(value, str):
                handle.write("\n".join(value))
            else:
                handle.write(str(value))
            handle.write(f"\n{delimiter}\n")
