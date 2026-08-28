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


class CoveragePayloadShapeError(TypeError):
    """Report that Rustdoc emitted a coverage payload other than an object."""


def aggregate_coverage_payload(per_file: object) -> Coverage:
    """Validate and sum Rustdoc's documented and total counts.

    Parameters
    ----------
    per_file
        Rustdoc's mapping from source-file names to coverage entries.

    Returns
    -------
    Coverage
        Aggregate documented and total item counts.

    Raises
    ------
    CoveragePayloadShapeError
        If Rustdoc's payload is not an object.
    KeyError, ValueError
        If an entry omits or violates a coverage-count invariant.
    """
    match per_file:
        case dict() as entries:
            return sum(
                (coverage_from_entry(entry) for entry in entries.values()),
                Coverage(0, 0),
            )
        case _:
            raise CoveragePayloadShapeError("expected an object")


def coverage_from_entry(entry: object) -> Coverage:
    """Validate one Rustdoc coverage entry and convert it to ``Coverage``.

    Parameters
    ----------
    entry
        Rustdoc entry containing ``total`` and ``with_docs`` counts.

    Returns
    -------
    Coverage
        Validated counts for one source file.

    Raises
    ------
    KeyError
        If either required count is absent.
    TypeError, ValueError
        If a count is not a non-negative integer or documented items exceed
        total items.
    """
    total = coverage_count(entry, "total")
    with_docs = coverage_count(entry, "with_docs")
    if with_docs > total:
        raise ValueError("counts must be non-negative integers with with_docs <= total")
    return Coverage(total, with_docs)


def coverage_count(entry: object, name: str) -> int:
    """Validate one named Rustdoc coverage count.

    Parameters
    ----------
    entry
        Rustdoc coverage entry containing the requested count.
    name
        Name of the count to retrieve.

    Returns
    -------
    int
        The validated non-negative count.

    Raises
    ------
    KeyError
        If ``name`` is absent from ``entry``.
    TypeError, ValueError
        If the count cannot be an integer count, including JSON booleans, or
        is negative.
    """
    count = entry[name]
    match count:
        case bool():
            raise ValueError(
                "counts must be non-negative integers with with_docs <= total"
            )
        case int() if count >= 0:
            return count
        case int():
            raise ValueError(
                "counts must be non-negative integers with with_docs <= total"
            )
        case _:
            raise ValueError(
                "counts must be non-negative integers with with_docs <= total"
            )
