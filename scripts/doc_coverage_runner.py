"""Select documentation targets and coordinate coverage measurements.

This module owns repository workflow policy and target discovery.
``doc_coverage_cargo`` owns Cargo and Rustdoc process handling, while the
command-line entry point owns argument parsing, reporting, and exit codes.
"""

from __future__ import annotations

import pathlib
import tomllib

from doc_coverage_cargo import load_metadata, measure
from doc_coverage_model import Coverage, DocTarget


def pinned_toolchain(manifest_root: pathlib.Path) -> str:
    """Return the channel pinned in the repository's toolchain file.

    Parameters
    ----------
    manifest_root
        Workspace root containing ``rust-toolchain.toml``.

    Returns
    -------
    str
        The configured dated Rust toolchain channel.

    Raises
    ------
    RuntimeError
        If the toolchain file cannot be read or lacks a channel.
    """
    try:
        with (manifest_root / "rust-toolchain.toml").open("rb") as toolchain:
            return tomllib.load(toolchain)["toolchain"]["channel"]
    except (OSError, tomllib.TOMLDecodeError, KeyError) as error:
        detail = f"cannot read the pinned toolchain from rust-toolchain.toml: {error}"
        raise RuntimeError(detail) from error


def doc_targets(metadata: dict[str, object]) -> list[DocTarget]:
    """Derive library and binary targets for every workspace member.

    Parameters
    ----------
    metadata
        Decoded Cargo metadata document for the workspace.

    Returns
    -------
    list[DocTarget]
        Every library and binary target belonging to a workspace member.

    Raises
    ------
    RuntimeError
        If Cargo metadata lacks the workspace package or member collections.
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


def doc_able_targets(
    package: dict[str, object], target: dict[str, object]
) -> list[DocTarget]:
    """Map one Cargo target to its measurable target, if any.

    Parameters
    ----------
    package
        Cargo metadata entry for the package that owns ``target``.
    target
        Cargo metadata entry describing one build target.

    Returns
    -------
    list[DocTarget]
        The library or binary target when it is measurable, otherwise empty.
    """
    kinds: list[str] = target.get("kind", [])
    if "lib" in kinds:
        return [DocTarget(package["name"], "lib", None)]
    if "bin" in kinds:
        return [DocTarget(package["name"], "bin", target["name"])]
    return []


def run_measurements(
    toolchain: str, manifest_root: pathlib.Path
) -> tuple[Coverage, list[tuple[DocTarget, Coverage]]]:
    """Measure every target and return aggregate plus target-specific coverage.

    Parameters
    ----------
    toolchain
        Dated nightly channel to select for every Cargo invocation.
    manifest_root
        Workspace root passed to metadata discovery and Rustdoc.

    Returns
    -------
    tuple[Coverage, list[tuple[DocTarget, Coverage]]]
        Aggregate coverage followed by each target and its coverage result.

    Raises
    ------
    RuntimeError
        If Cargo metadata discovery or any target measurement fails.
    """
    totals = Coverage(0, 0)
    rows: list[tuple[DocTarget, Coverage]] = []
    for target in doc_targets(load_metadata(toolchain, manifest_root)):
        coverage = measure(target, toolchain, manifest_root)
        rows.append((target, coverage))
        totals += coverage
    return totals, rows
