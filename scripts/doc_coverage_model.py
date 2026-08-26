"""Represent and validate the data used by the Rustdoc coverage gate.

The command-line script owns process execution and user-facing diagnostics;
this module owns the pure coverage value objects and payload validation. The
split keeps both modules below the repository's source-file size limit while
leaving the executable's public import surface unchanged.
"""

from __future__ import annotations

import dataclasses as dc


@dc.dataclass(frozen=True)
class Coverage:
    """Counts of Rustdoc-measured items for one documentation run."""

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
    """Describe one library or binary target measured by Rustdoc."""

    package: str
    kind: str
    name: str | None


def aggregate_coverage_payload(per_file: object) -> Coverage:
    """Validate and sum Rustdoc's documented and total counts."""
    match per_file:
        case dict() as entries:
            return sum(
                (coverage_from_entry(entry) for entry in entries.values()),
                Coverage(0, 0),
            )
        case _:
            raise TypeError("expected an object")


def coverage_from_entry(entry: object) -> Coverage:
    """Validate one Rustdoc coverage entry and convert it to `Coverage`."""
    total = coverage_count(entry, "total")
    with_docs = coverage_count(entry, "with_docs")
    if with_docs > total:
        raise ValueError("counts must be non-negative integers with with_docs <= total")
    return Coverage(total, with_docs)


def coverage_count(entry: object, name: str) -> int:
    """Convert and validate one Rustdoc coverage count."""
    count = entry[name]
    if isinstance(count, bool):
        raise ValueError("counts must be non-negative integers with with_docs <= total")
    if not isinstance(count, int):
        raise ValueError("counts must be non-negative integers with with_docs <= total")
    if count < 0:
        raise ValueError("counts must be non-negative integers with with_docs <= total")
    return count
