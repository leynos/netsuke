"""Select documentation targets and coordinate coverage measurements.

This module owns repository workflow policy and target discovery.
``doc_coverage_cargo`` owns Cargo and Rustdoc process handling, while the
command-line entry point owns argument parsing, reporting, and exit codes.
"""

import tomllib
import typing as typ

import doc_coverage_cargo
from doc_coverage_model import Coverage, DocTarget

if typ.TYPE_CHECKING:
    import pathlib


class ToolchainPinError(RuntimeError):
    """Report an unreadable or channel-less ``rust-toolchain.toml``."""

    INVALID_RECORD = "toolchain.channel must be a non-empty string"

    def __init__(self, detail: str) -> None:
        super().__init__(
            f"cannot read the pinned toolchain from rust-toolchain.toml: {detail}"
        )


class WorkspaceMetadataError(RuntimeError):
    """Report Cargo metadata that omits the workspace packages or members."""

    def __init__(self) -> None:
        super().__init__(
            "cargo metadata response lacks the workspace packages or members"
        )


class CoverageAdapter(typ.Protocol):
    """Define the Cargo boundary consumed by measurement orchestration."""

    def load_metadata(
        self, toolchain: str, manifest_root: pathlib.Path
    ) -> dict[str, object]:
        """Load Cargo metadata for the selected workspace."""

    def measure(
        self, target: DocTarget, toolchain: str, manifest_root: pathlib.Path
    ) -> Coverage:
        """Measure documentation coverage for one selected target."""


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
    ToolchainPinError
        If the toolchain file cannot be read or lacks a channel.
    """
    try:
        with (manifest_root / "rust-toolchain.toml").open("rb") as toolchain:
            record = tomllib.load(toolchain)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise ToolchainPinError(str(error)) from error
    match record:
        case {"toolchain": {"channel": str() as channel}} if channel:
            return channel
        case _:
            raise ToolchainPinError(ToolchainPinError.INVALID_RECORD)


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
    WorkspaceMetadataError
        If Cargo metadata lacks the workspace package or member collections.
    """
    match metadata:
        case {
            "workspace_members": list() as raw_members,
            "packages": list() as packages,
        }:
            members: set[str] = set()
            for member in raw_members:
                match member:
                    case str():
                        members.add(member)
                    case _:
                        raise WorkspaceMetadataError
        case _:
            raise WorkspaceMetadataError
    return [doc for package in packages for doc in package_targets(package, members)]


def package_targets(package: object, members: set[str]) -> list[DocTarget]:
    """Map one Cargo package entry to the targets worth documenting.

    Parameters
    ----------
    package
        Cargo metadata entry for one package, in its decoded JSON form.
    members
        Package identifiers belonging to the workspace under measurement.

    Returns
    -------
    list[DocTarget]
        Measurable targets of a workspace member, otherwise empty.
    """
    match package:
        case {
            "id": str() as identifier,
            "name": str() as name,
            "targets": list() as targets,
        } if identifier in members:
            return [doc for target in targets for doc in doc_able_targets(name, target)]
        case _:
            return []


def doc_able_targets(package: str, target: object) -> list[DocTarget]:
    """Map one Cargo target to its measurable target, if any.

    Parameters
    ----------
    package
        Name of the Cargo package that owns ``target``.
    target
        Cargo metadata entry describing one build target.

    Returns
    -------
    list[DocTarget]
        The library or binary target when it is measurable, otherwise empty.
    """
    match target:
        case {"kind": list() as kinds} if "lib" in kinds:
            return [DocTarget(package, "lib", None)]
        case {"kind": list() as kinds, "name": str() as name} if "bin" in kinds:
            return [DocTarget(package, "bin", name)]
        case _:
            return []


def run_measurements(
    toolchain: str,
    manifest_root: pathlib.Path,
    adapter: CoverageAdapter | None = None,
) -> tuple[Coverage, list[tuple[DocTarget, Coverage]]]:
    """Measure every target and return aggregate plus target-specific coverage.

    Parameters
    ----------
    toolchain
        Dated nightly channel to select for every Cargo invocation.
    manifest_root
        Workspace root passed to metadata discovery and Rustdoc.
    adapter
        Cargo and Rustdoc boundary to use. When omitted, the configured
        production Cargo adapter is constructed.

    Returns
    -------
    tuple[Coverage, list[tuple[DocTarget, Coverage]]]
        Aggregate coverage followed by each target and its coverage result.

    Notes
    -----
    Measurement failures surface as ``RuntimeError`` subclasses propagated from
    target discovery and from the Cargo adapter; the command-line entry point
    turns them into its diagnostic and exit code.
    """
    coverage_adapter = adapter or doc_coverage_cargo.production_adapter()
    totals = Coverage(0, 0)
    rows: list[tuple[DocTarget, Coverage]] = []
    for target in doc_targets(coverage_adapter.load_metadata(toolchain, manifest_root)):
        coverage = coverage_adapter.measure(target, toolchain, manifest_root)
        rows.append((target, coverage))
        totals += coverage
    return totals, rows
