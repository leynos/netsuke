"""Adapt Cargo and Rustdoc commands for the documentation-coverage gate.

This module owns process invocation and Rustdoc's generated coverage artefact.
``doc_coverage_runner`` owns repository policy and target selection, while the
command-line entry point owns argument parsing, reporting, and exit codes.
"""

import dataclasses as dc
import json
import os
import pathlib

# Driving Cargo and Rustdoc as child processes is this module's whole purpose.
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the boundary is deliberate.

from doc_coverage_model import Coverage, DocTarget

_COUNT_INVARIANT = "counts must be non-negative integers with with_docs <= total"


@dc.dataclass(frozen=True, slots=True)
class CargoAdapter:
    """Adapt one explicit Cargo executable to coverage measurements.

    The runner depends on this narrow interface instead of process globals, so
    its target selection and aggregation can be tested without subprocesses.
    """

    executable: str

    def load_metadata(
        self, toolchain: str, manifest_root: pathlib.Path
    ) -> dict[str, object]:
        """Load workspace metadata through this adapter's Cargo executable."""
        return load_metadata(toolchain, manifest_root, self.executable)

    def measure(
        self, target: DocTarget, toolchain: str, manifest_root: pathlib.Path
    ) -> Coverage:
        """Measure one target through this adapter's Cargo executable."""
        return measure(target, toolchain, manifest_root, self.executable)


class CoveragePayloadShapeError(TypeError):
    """Report that Rustdoc emitted a coverage payload other than an object."""

    def __init__(self) -> None:
        super().__init__("expected an object")


class CoverageEntryShapeError(ValueError):
    """Report a Rustdoc coverage entry missing a required count."""

    def __init__(self) -> None:
        super().__init__("entry must provide total and with_docs")


class CoverageCountError(ValueError):
    """Report a Rustdoc coverage count that violates the count invariants."""

    def __init__(self) -> None:
        super().__init__(_COUNT_INVARIANT)


class CoverageOutputError(RuntimeError):
    """Report that Rustdoc produced no usable coverage JSON for a target."""

    def __init__(self, target: DocTarget, detail: str) -> None:
        super().__init__(
            f"cargo rustdoc for {target.package} {target.kind}"
            f" ({target.name or 'lib'}) did not emit coverage JSON: {detail}"
        )


def production_adapter() -> CargoAdapter:
    """Create the production adapter with the configured Cargo executable."""
    return CargoAdapter(os.environ.get("CARGO") or "cargo")


def rustdoc_args(target: DocTarget, toolchain: str, cargo_executable: str) -> list[str]:
    """Build Cargo's Rustdoc coverage command for one target.

    Parameters
    ----------
    target
        Workspace target Cargo will document.
    toolchain
        Dated nightly channel to select with Cargo's ``+`` syntax.
    cargo_executable
        Cargo executable selected by the adapter at the production boundary.

    Returns
    -------
    list[str]
        Argument vector for ``cargo rustdoc`` with its coverage options.
    """
    args = [cargo_executable, f"+{toolchain}", "rustdoc", "-p", target.package]
    match target:
        case DocTarget(kind="bin", name=str() as binary):
            args += ["--bin", binary]
        case _:
            args.append("--lib")
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
    """Decode and aggregate one generated Rustdoc coverage payload.

    Parameters
    ----------
    target
        Workspace target whose Rustdoc output is being decoded.
    output
        Generated coverage JSON payload.

    Returns
    -------
    Coverage
        Aggregate documented and total item counts.

    Raises
    ------
    CoverageOutputError
        If the payload is malformed or does not contain valid coverage counts.
    """
    try:
        per_file = json.loads(output)
    except json.JSONDecodeError as error:
        raise CoverageOutputError(target, str(error)) from error
    try:
        return aggregate_coverage_payload(per_file)
    except CoveragePayloadShapeError as error:
        raise CoverageOutputError(target, str(error)) from error
    except ValueError as error:
        detail = f"each entry requires total and with_docs: {error}"
        raise CoverageOutputError(target, detail) from error


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

    Notes
    -----
    Per-entry validation is delegated to :func:`coverage_from_entry`, so
    :class:`CoverageEntryShapeError` and :class:`CoverageCountError` propagate
    from here whenever an entry violates a coverage-count invariant.
    """
    match per_file:
        case dict() as entries:
            return sum(
                (coverage_from_entry(entry) for entry in entries.values()),
                Coverage(0, 0),
            )
        case _:
            raise CoveragePayloadShapeError


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
    CoverageEntryShapeError
        If the entry is not an object carrying both required counts.
    CoverageCountError
        If a count is not a non-negative integer, or documented items exceed
        total items.
    """
    match entry:
        case {"total": raw_total, "with_docs": raw_with_docs}:
            total = coverage_count(raw_total)
            with_docs = coverage_count(raw_with_docs)
        case _:
            raise CoverageEntryShapeError
    if with_docs > total:
        raise CoverageCountError
    return Coverage(total, with_docs)


def coverage_count(count: object) -> int:
    """Validate one Rustdoc coverage count.

    Parameters
    ----------
    count
        Decoded JSON value for a ``total`` or ``with_docs`` count.

    Returns
    -------
    int
        The validated non-negative count.

    Raises
    ------
    CoverageCountError
        If the value is not an integer count, including JSON booleans and
        non-finite floats, or if it is negative.
    """
    match count:
        # JSON booleans decode to ``bool``, which is an ``int`` subclass, so
        # they must be rejected before the integer arm accepts them.
        case bool():
            raise CoverageCountError
        case int() if count >= 0:
            return count
        case _:
            raise CoverageCountError


def coverage_output_path(
    target: DocTarget, output: str, manifest_root: pathlib.Path
) -> pathlib.Path:
    """Return the generated coverage JSON path reported by Rustdoc.

    Parameters
    ----------
    target
        Workspace target whose Rustdoc output is being inspected.
    output
        Rustdoc standard output containing its generated-file notice.
    manifest_root
        Workspace root used to resolve a relative generated-file path.

    Returns
    -------
    pathlib.Path
        Absolute path to the generated coverage JSON file.

    Raises
    ------
    CoverageOutputError
        If Rustdoc does not report a usable generated coverage JSON path.
    """
    prefix = 'Generated output into "'
    for line in output.splitlines():
        if line.startswith(prefix) and line.endswith('"'):
            reported_path = line.removeprefix(prefix).removesuffix('"')
            if reported_path:
                path = pathlib.Path(reported_path)
                return path if path.is_absolute() else manifest_root / path
    detail = "Rustdoc did not report the generated coverage JSON path"
    raise CoverageOutputError(target, detail)


def measure(
    target: DocTarget,
    toolchain: str,
    manifest_root: pathlib.Path,
    cargo_executable: str,
) -> Coverage:
    """Run Rustdoc coverage for one target and sum its per-file counts.

    Parameters
    ----------
    target
        Workspace target to document.
    toolchain
        Dated nightly channel to select for Cargo.
    manifest_root
        Workspace root passed to Cargo and used to resolve generated output.
    cargo_executable
        Explicit Cargo executable for the Rustdoc process.

    Returns
    -------
    Coverage
        Aggregate documented and total item counts for ``target``.

    Raises
    ------
    RuntimeError
        If Cargo cannot be run or exits non-zero.
    CoverageOutputError
        If Rustdoc omits the reported path or emits invalid coverage JSON.
    """
    try:
        # Cargo metadata targets and the pinned toolchain compose argv, so no
        # untrusted input reaches the child process.
        result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - shell is False.
            rustdoc_args(target, toolchain, cargo_executable),
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
    output_path = coverage_output_path(target, result.stdout, manifest_root)
    try:
        output = output_path.read_text(encoding="utf-8")
    except OSError as error:
        detail = f"cannot read generated coverage JSON at {output_path}: {error}"
        raise CoverageOutputError(target, detail) from error
    return parse_coverage_output(target, output)


def load_metadata(
    toolchain: str, manifest_root: pathlib.Path, cargo_executable: str
) -> dict[str, object]:
    """Return Cargo metadata for the workspace rooted at ``manifest_root``.

    Parameters
    ----------
    toolchain
        Dated nightly channel to select for Cargo.
    manifest_root
        Workspace root from which Cargo reads its manifest.
    cargo_executable
        Explicit Cargo executable for the metadata process.

    Returns
    -------
    dict[str, object]
        Decoded ``cargo metadata --format-version 1`` document.

    Raises
    ------
    RuntimeError
        If Cargo cannot run, returns failure, or emits invalid JSON.
    """
    args = [
        cargo_executable,
        f"+{toolchain}",
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
    ]
    try:
        # The Cargo executable and a fixed metadata flag list compose argv, so
        # no untrusted input reaches the child process.
        result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - shell is False.
            args,
            cwd=manifest_root,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
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
