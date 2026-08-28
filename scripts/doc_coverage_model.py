"""Represent the values shared by the Rustdoc documentation-coverage gate.

The Cargo adapter owns Rustdoc payload decoding and validation. This module
keeps only the values shared by adapter, orchestration, and command-line code.
"""

from __future__ import annotations

import dataclasses as dc


@dc.dataclass(frozen=True)
class Coverage:
    """Represent Rustdoc-measured items for one documentation run.

    Parameters
    ----------
    total
        Total number of Rustdoc-counted items.
    with_docs
        Number of those items carrying documentation.
    """

    total: int
    with_docs: int

    @property
    def percentage(self) -> float:
        """Return the share of documented items as a percentage."""
        return 100.0 * self.with_docs / self.total if self.total else 100.0

    def __add__(self, other: Coverage) -> Coverage:
        """Return the sum of two coverage counts."""
        return Coverage(self.total + other.total, self.with_docs + other.with_docs)


@dc.dataclass(frozen=True)
class DocTarget:
    """Describe one library or binary target measured by Rustdoc.

    Parameters
    ----------
    package
        Cargo package containing the target.
    kind
        Target category: ``"lib"`` or ``"bin"``.
    name
        Binary target name, or ``None`` for a library target.
    """

    package: str
    kind: str
    name: str | None
