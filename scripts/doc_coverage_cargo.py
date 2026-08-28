"""Adapt Cargo and Rustdoc commands for the documentation-coverage gate.

This module owns process invocation and Rustdoc's generated coverage artefact.
``doc_coverage_runner`` owns repository policy and target selection, while the
command-line entry point owns argument parsing, reporting, and exit codes.
"""

from __future__ import annotations

import json
import os
import pathlib
import subprocess
import dataclasses as dc

from doc_coverage_model import Coverage, DocTarget


@dc.dataclass(frozen=True)
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
    RuntimeError
        If the payload is malformed or does not contain valid coverage counts.
    """
    try:
        per_file = json.loads(output)
    except json.JSONDecodeError as error:
        raise coverage_json_error(target, str(error)) from error
    try:
        return aggregate_coverage_payload(per_file)
    except CoveragePayloadShapeError as error:
        raise coverage_json_error(target, str(error)) from error
    except (KeyError, TypeError, ValueError, OverflowError) as error:
        detail = f"each entry requires total and with_docs: {error}"
        raise coverage_json_error(target, detail) from error


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


def coverage_json_error(target: DocTarget, detail: str) -> RuntimeError:
    """Build a measurement error naming its target and coverage-output detail.

    Parameters
    ----------
    target
        Workspace target whose output Rustdoc produced.
    detail
        Explanation of why the generated coverage output is invalid.

    Returns
    -------
    RuntimeError
        Unraised error ready to identify the target at the caller boundary.
    """
    message = (
        f"cargo rustdoc for {target.package} {target.kind}"
        f" ({target.name or 'lib'}) did not emit coverage JSON: {detail}"
    )
    return RuntimeError(message)


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
    RuntimeError
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
    raise coverage_json_error(target, detail)


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
        If Cargo or Rustdoc fails, omits the reported path, or emits invalid
        coverage JSON.
    """
    try:
        result = subprocess.run(  # noqa: S603 - Cargo metadata targets and pinned toolchain form argv; shell remains False.
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
        raise coverage_json_error(target, detail) from error
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
        result = subprocess.run(  # noqa: S603 - Cargo metadata targets and pinned toolchain form argv; shell remains False.
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
