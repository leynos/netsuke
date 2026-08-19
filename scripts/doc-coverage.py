#!/usr/bin/env python3
"""Compute aggregate Rustdoc doc-comment coverage across the workspace.

Runs ``cargo rustdoc --show-coverage`` for every library and binary target
of every workspace member, sums each target's documented and total items,
and reports the aggregate against a pass threshold. Private items are counted
because the coverage bar applies to them too: the metric feeds the
``make doc-coverage`` gate described in AGENTS.md.

Rustdoc's own counting excludes trait-implementation items (``Display::fmt``,
``FromStr::from_str``, ``Serialize``, ``Deserialize``, ``Drop::drop`` and
friends) and inherent `impl`-block methods, so those never need a ``///``
doc comment to satisfy the metric.

The command returns non-zero when the aggregate falls below ``--threshold``,
which is what CI gates on. It also prints a per-target breakdown so a
remediation sweep can target the lowest-coverage files first.
"""

from __future__ import annotations

import argparse
import dataclasses as dc
import json
import pathlib
import subprocess
import sys
import tomllib
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent


@dc.dataclass(frozen=True)
class Coverage:
    """Counts of Rustdoc-measured items for one documentation run."""

    total: int
    with_docs: int

    @property
    def percentage(self) -> float:
        """Return the share of documented items as a percentage.

        An empty run is treated as complete rather than dividing by zero; a
        crate with no doc-able targets contributes nothing either way.
        """
        return 100.0 * self.with_docs / self.total if self.total else 100.0

    def __add__(self, other: Coverage) -> Coverage:
        """Return the sum of two coverage counts."""
        return Coverage(self.total + other.total, self.with_docs + other.with_docs)


@dc.dataclass(frozen=True)
class DocTarget:
    """One target among a package's doc-able (library or binary) targets.

    Parameters
    ----------
    package
        Cargo package name the target belongs to.
    kind
        ``"lib"`` for a library target, ``"bin"`` for a binary target.
    name
        Binary target name, or ``None`` for a library.
    """

    package: str
    kind: str
    name: str | None


def pinned_toolchain(manifest_root: pathlib.Path) -> str:
    """Return the ``channel`` pinned in the repository's toolchain file."""
    with (manifest_root / "rust-toolchain.toml").open("rb") as toolchain:
        return tomllib.load(toolchain)["toolchain"]["channel"]


def doc_targets(metadata: dict) -> list[DocTarget]:
    """Derive the library and binary targets of every workspace member.

    Membership is taken from ``workspace_members`` so dependency crates
    outside the workspace are never measured. Build scripts, integration
    tests, examples, and benches are skipped: Rustdoc coverage is defined for
    the shipped library and binary surfaces, and test code is excluded by
    repo convention (see AGENTS.md).
    """
    members = set(metadata["workspace_members"])
    targets: list[DocTarget] = []
    for package in metadata["packages"]:
        if package["id"] not in members:
            continue
        for target in package["targets"]:
            kinds: list[str] = target["kind"]
            if "lib" in kinds:
                targets.append(DocTarget(package["name"], "lib", None))
            elif "bin" in kinds:
                targets.append(DocTarget(package["name"], "bin", target["name"]))
    return targets


def measure(target: DocTarget, toolchain: str, manifest_root: pathlib.Path) -> Coverage:
    """Run Rustdoc's coverage meter for one target and sum its per-file counts.

    ``RUSTFLAGS`` and ``RUSTDOCFLAGS`` flow through from the environment so
    the Makefile can thread the Polonius flag and the docsrs/deny-warnings
    policy that the rest of the tree builds with.
    """
    args = ["cargo", f"+{toolchain}", "rustdoc", "-p", target.package]
    if target.kind == "bin":
        args += ["--bin", target.name]
    else:
        args += ["--lib"]
    args += [
        "--",
        "-Z",
        "unstable-options",
        "--show-coverage",
        "--output-format",
        "json",
        "--document-private-items",
    ]
    # With no shell involved and argv built from workspace metadata plus
    # constant flags, there is no untrusted input to inject.
    result = subprocess.run(  # noqa: S603
        args,
        cwd=manifest_root,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        detail = (
            f"cargo rustdoc failed for {target.package} {target.kind}"
            f" ({target.name or 'lib'}):\n{result.stderr}"
        )
        raise RuntimeError(detail)
    try:
        per_file = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        detail = (
            f"cargo rustdoc for {target.package} {target.kind}"
            f" ({target.name or 'lib'}) did not emit coverage JSON: {error}"
        )
        raise RuntimeError(detail) from error
    return sum(
        (
            Coverage(int(entry["total"]), int(entry["with_docs"]))
            for entry in per_file.values()
        ),
        Coverage(0, 0),
    )


def label(target: DocTarget) -> str:
    """Return a human-readable name for the target in the breakdown table.

    Binary targets are labelled with their target name; libraries use the
    package name alone.
    """
    if not target.name:
        return f"{target.package} {target.kind}"
    return f"{target.package} {target.kind} ({target.name})"


def run_measurements(
    toolchain: str, manifest_root: pathlib.Path
) -> tuple[Coverage, list[tuple[str, Coverage]]]:
    """Measure every doc target and return the aggregate and per-target rows.

    A failed ``cargo rustdoc`` or missing JSON output aborts the whole run —
    a broken measurement is worse than an unmeasured one.
    """
    totals = Coverage(0, 0)
    rows: list = []
    for target in doc_targets(load_metadata(toolchain, manifest_root)):
        coverage = measure(target, toolchain, manifest_root)
        rows.append((label(target), coverage))
        totals += coverage
    return totals, rows


def load_metadata(toolchain: str, manifest_root: pathlib.Path) -> dict:
    """Return the ``cargo metadata`` document for the workspace."""
    args = [
        "cargo",
        f"+{toolchain}",
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
    ]
    # The argv is a static command with the pinned toolchain and metadata
    # flags; there is no shell or injection surface.
    result = subprocess.run(  # noqa: S603
        args,
        cwd=manifest_root,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        detail = f"cargo metadata failed: {result.stderr}"
        raise RuntimeError(detail)
    return json.loads(result.stdout)


def main(argv: cabc.Sequence[str] | None = None) -> int:
    """Run the coverage gate and return the process exit code."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--threshold",
        type=float,
        default=80.0,
        help="minimum aggregate percentage; exit non-zero below this (default: 80)",
    )
    parser.add_argument(
        "--toolchain",
        default=None,
        help="override the channel pinned in rust-toolchain.toml",
    )
    args = parser.parse_args(argv)

    manifest_root = REPO_ROOT
    toolchain = args.toolchain or pinned_toolchain(manifest_root)
    try:
        totals, rows = run_measurements(toolchain, manifest_root)
    except RuntimeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    for name, coverage in rows:
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
