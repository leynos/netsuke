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

from doc_coverage_model import (
    Coverage,
    CoveragePayloadShapeError,
    DocTarget,
    aggregate_coverage_payload,
)


def cargo_executable() -> str:
    """Return the configured Cargo executable.

    Returns
    -------
    str
        The value of ``CARGO``, or ``"cargo"`` when it is unset.
    """
    return os.environ.get("CARGO") or "cargo"


def rustdoc_args(target: DocTarget, toolchain: str) -> list[str]:
    """Build Cargo's Rustdoc coverage command for one target.

    Parameters
    ----------
    target
        Workspace target Cargo will document.
    toolchain
        Dated nightly channel to select with Cargo's ``+`` syntax.

    Returns
    -------
    list[str]
        Argument vector for ``cargo rustdoc`` with its coverage options.
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


def measure(target: DocTarget, toolchain: str, manifest_root: pathlib.Path) -> Coverage:
    """Run Rustdoc coverage for one target and sum its per-file counts.

    Parameters
    ----------
    target
        Workspace target to document.
    toolchain
        Dated nightly channel to select for Cargo.
    manifest_root
        Workspace root passed to Cargo and used to resolve generated output.

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
    output_path = coverage_output_path(target, result.stdout, manifest_root)
    try:
        output = output_path.read_text(encoding="utf-8")
    except OSError as error:
        detail = f"cannot read generated coverage JSON at {output_path}: {error}"
        raise coverage_json_error(target, detail) from error
    return parse_coverage_output(target, output)


def load_metadata(toolchain: str, manifest_root: pathlib.Path) -> dict[str, object]:
    """Return Cargo metadata for the workspace rooted at ``manifest_root``.

    Parameters
    ----------
    toolchain
        Dated nightly channel to select for Cargo.
    manifest_root
        Workspace root from which Cargo reads its manifest.

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
        cargo_executable(),
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
