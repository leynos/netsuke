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
import dataclasses as dc
import json
import os
import pathlib
import subprocess
import sys
import tomllib
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent


def cargo_executable() -> str:
    """Return the configured Cargo executable.

    The Makefile exposes a ``CARGO`` override that every Cargo-backed target
    honours, so the coverage script reads it from the environment and falls
    back to ``cargo`` on ``PATH``. Hard-coding the program would run the wrong
    wrapper or tool installation wherever the Makefile is invoked with a
    custom executable.
    """
    return os.environ.get("CARGO") or "cargo"


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
    try:
        with (manifest_root / "rust-toolchain.toml").open("rb") as toolchain:
            return tomllib.load(toolchain)["toolchain"]["channel"]
    except (OSError, tomllib.TOMLDecodeError, KeyError) as error:
        detail = f"cannot read the pinned toolchain from rust-toolchain.toml: {error}"
        raise RuntimeError(detail) from error


def doc_targets(metadata: dict) -> list[DocTarget]:
    """Derive the library and binary targets of every workspace member.

    Membership is taken from ``workspace_members`` so dependency crates
    outside the workspace are never measured. Build scripts, integration
    tests, examples, and benches are skipped: Rustdoc coverage is defined for
    the shipped library and binary surfaces, and test code is excluded by
    repo convention (see AGENTS.md).

    The expected shape comes from ``cargo metadata --format-version 1``. A
    response that is valid JSON but lacks the workspace keys is a broken
    measurement, so it is rejected with an explicit error rather than crashing
    on a ``KeyError``. An individual package record that omits its ``id`` or
    ``targets`` keys simply contributes no targets to the aggregate.
    """
    try:
        members = set(metadata["workspace_members"])
        packages = metadata["packages"]
    except (KeyError, TypeError) as error:
        detail = "cargo metadata response lacks the workspace packages or members"
        raise RuntimeError(detail) from error
    return [
        doc
        for package in packages
        if "id" in package and "targets" in package
        if package["id"] in members
        for ordinal in package["targets"]
        for doc in doc_able_targets(package, ordinal)
    ]


def doc_able_targets(package: dict, target: dict) -> list[DocTarget]:
    """Map one package ordinal target to a doc target, or none.

    Returns an empty list for targets that do not count toward Rustdoc
    coverage: build scripts, tests, examples, and benches. Cargo reports a
    target's kinds as a list, so both `lib` and `bin` can be matched without
    guessing at the shape of a single-kind target.
    """
    kinds: list[str] = target.get("kind", [])
    if "lib" in kinds:
        return [DocTarget(package["name"], "lib", None)]
    if "bin" in kinds:
        return [DocTarget(package["name"], "bin", target["name"])]
    return []


def rustdoc_args(target: DocTarget, toolchain: str) -> list[str]:
    """Build the cargo rustdoc coverage command for one target.

    Parameters
    ----------
    target
        The library or binary target to measure.
    toolchain
        The ``+channel`` selector passed to Cargo.

    Returns
    -------
    list[str]
        The complete argument vector, starting with the ``cargo +<toolchain>
        rustdoc -p <package>`` invocation, followed by the library or binary
        selector and then the Rustdoc coverage flags in their fixed order.
    """
    args = [cargo_executable(), f"+{toolchain}", "rustdoc", "-p", target.package]
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
    return args


def parse_coverage_output(target: DocTarget, output: str) -> Coverage:
    """Decode one rustdoc coverage payload and sum its per-file counts.

    Parameters
    ----------
    target
        The target whose JSON payload is parsed; named only in the error
        diagnostic so failures identify the measured crate.
    output
        The ``--show-coverage --output-format json`` document rustdoc wrote
        to stdout.

    Returns
    -------
    Coverage
        The aggregate ``total`` and ``with_docs`` counts across every file
        entry in the payload.

    Raises
    ------
    RuntimeError
        When ``output`` is invalid JSON or does not have Rustdoc's per-file
        object shape, with the ``did not emit coverage JSON`` diagnostic
        naming the target.
    """
    try:
        per_file = json.loads(output)
    except json.JSONDecodeError as error:
        detail = (
            f"cargo rustdoc for {target.package} {target.kind}"
            f" ({target.name or 'lib'}) did not emit coverage JSON: {error}"
        )
        raise RuntimeError(detail) from error
    if not isinstance(per_file, dict):
        detail = (
            f"cargo rustdoc for {target.package} {target.kind}"
            f" ({target.name or 'lib'}) did not emit coverage JSON: expected an object"
        )
        raise RuntimeError(detail)
    try:
        return sum(
            (
                Coverage(int(entry["total"]), int(entry["with_docs"]))
                for entry in per_file.values()
            ),
            Coverage(0, 0),
        )
    except (KeyError, TypeError, ValueError) as error:
        detail = (
            f"cargo rustdoc for {target.package} {target.kind}"
            f" ({target.name or 'lib'}) did not emit coverage JSON: "
            f"each entry requires total and with_docs: {error}"
        )
        raise RuntimeError(detail) from error


def measure(target: DocTarget, toolchain: str, manifest_root: pathlib.Path) -> Coverage:
    """Run Rustdoc's coverage meter for one target and sum its per-file counts.

    ``RUSTFLAGS`` and ``RUSTDOCFLAGS`` flow through from the environment so
    the Makefile can thread the Polonius flag and the docsrs/deny-warnings
    policy that the rest of the tree builds with.
    """
    # With no shell involved and argv built from workspace metadata plus
    # constant flags, there is no untrusted input to inject.
    try:
        result = subprocess.run(  # noqa: S603
            rustdoc_args(target, toolchain),
            cwd=manifest_root,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        detail = (
            f"cannot run cargo rustdoc for {target.package} {target.kind}"
            f" ({target.name or 'lib'}): {error}"
        )
        raise RuntimeError(detail) from error
    if result.returncode != 0:
        detail = (
            f"cargo rustdoc failed for {target.package} {target.kind}"
            f" ({target.name or 'lib'}):\n{result.stderr}"
        )
        raise RuntimeError(detail)
    return parse_coverage_output(target, result.stdout)


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
        cargo_executable(),
        f"+{toolchain}",
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
    ]
    # The argv is a static command with the pinned toolchain and metadata
    # flags; there is no shell or injection surface.
    try:
        result = subprocess.run(  # noqa: S603
            args,
            cwd=manifest_root,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        # Missing or non-executable cargo must surface as an explicit
        # measurement error with the script's controlled exit code rather
        # than escaping as a bare traceback.
        detail = f"cannot run cargo metadata: {error}"
        raise RuntimeError(detail) from error
    if result.returncode != 0:
        detail = f"cargo metadata failed: {result.stderr}"
        raise RuntimeError(detail)
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        detail = f"cargo metadata emitted invalid JSON: {error}"
        raise RuntimeError(detail) from error


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
        toolchain = args.toolchain or pinned_toolchain(manifest_root)
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
