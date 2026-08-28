#!/usr/bin/env python3
"""Compute aggregate Rustdoc doc-comment coverage across the workspace.

Runs ``cargo rustdoc --show-coverage`` for every library and binary target
of every workspace member, sums each target's documented and total items,
and reports the aggregate against a pass threshold. Private items are counted
because the coverage bar applies to them too: the metric feeds the
``make doc-coverage`` gate described in AGENTS.md.

Rustdoc's own counting excludes trait-implementation items, so a concrete
``Display::fmt``, ``FromStr::from_str``, ``Serialize``, ``Deserialize``,
``Drop::drop`` or similar override never needs a ``///`` doc comment to
satisfy the metric. Inherent `impl`-block methods count like any other
function. Every module also counts, so a missing ``//!`` header on a module
lowers the aggregate.

The command returns non-zero when the aggregate falls below ``--threshold``,
which is what CI gates on. It also prints a per-target breakdown so a
remediation sweep can target the lowest-coverage files first.
"""

from __future__ import annotations

import argparse
import pathlib
import sys
import typing as typ

import doc_coverage_runner as runner
from doc_coverage_model import DocTarget

if typ.TYPE_CHECKING:
    import collections.abc as cabc

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent


def label(target: DocTarget) -> str:
    """Return a human-readable name for the target in the breakdown table.

    Binary targets are labelled with their target name; libraries use the
    package name alone.
    """
    if not target.name:
        return f"{target.package} {target.kind}"
    return f"{target.package} {target.kind} ({target.name})"


def parse_threshold(value: str) -> float:
    """Parse a coverage threshold, rejecting NaN and out-of-range values."""
    try:
        threshold = float(value)
    except ValueError as error:
        detail = f"invalid threshold {value!r}"
        raise argparse.ArgumentTypeError(detail) from error
    if not 0.0 <= threshold <= 100.0:
        detail = f"threshold must be in [0, 100], got {threshold}"
        raise argparse.ArgumentTypeError(detail)
    return threshold


def main(argv: cabc.Sequence[str] | None = None) -> int:
    """Run the coverage gate and return the process exit code."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--threshold",
        type=parse_threshold,
        default=80.0,
        help="minimum aggregate percentage; exit non-zero below this (default: 80)",
    )
    parser.add_argument(
        "--toolchain",
        default=None,
        help="override the channel pinned in rust-toolchain.toml",
    )
    parser.add_argument(
        "--manifest-root",
        type=pathlib.Path,
        default=REPO_ROOT,
        help="measure the workspace rooted here instead of the repository root "
        "(testing seam)",
    )
    args = parser.parse_args(argv)

    manifest_root = args.manifest_root
    try:
        toolchain = args.toolchain or runner.pinned_toolchain(manifest_root)
        totals, rows = runner.run_measurements(toolchain, manifest_root)
    except RuntimeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    for target, coverage in rows:
        name = label(target)
        print(
            f"{name:42s} {coverage.with_docs:5d}/{coverage.total:<5d} "
            f"{coverage.percentage:6.2f}%"
        )
    print(
        f"{'aggregate':42s} {totals.with_docs:5d}/{totals.total:<5d} "
        f"{totals.percentage:6.2f}%"
    )

    if totals.percentage < args.threshold:
        message = (
            f"doc-comment coverage {totals.percentage:.2f}% is below the "
            f"{args.threshold:.2f}% threshold; document the lowest-coverage "
            "targets listed above and re-run `make doc-coverage`."
        )
        print(message, file=sys.stderr)
        return 1
    print(
        f"ok: doc-comment coverage {totals.percentage:.2f}% meets the "
        f"{args.threshold:.2f}% threshold."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
