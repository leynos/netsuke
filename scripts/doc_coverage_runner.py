"""Run Cargo and Rustdoc for the documentation-coverage gate.

This module owns the process boundary and generated-Rustdoc-output handling.
The command-line entry point owns argument parsing, reporting, and exit codes;
``doc_coverage_model`` owns pure coverage values and payload validation.
"""

from __future__ import annotations

import json
import os
import pathlib
import subprocess
import tomllib

from doc_coverage_model import Coverage, DocTarget, aggregate_coverage_payload


def cargo_executable() -> str:
    """Return the configured Cargo executable."""
    return os.environ.get("CARGO") or "cargo"


def pinned_toolchain(manifest_root: pathlib.Path) -> str:
    """Return the channel pinned in the repository's toolchain file."""
    try:
        with (manifest_root / "rust-toolchain.toml").open("rb") as toolchain:
            return tomllib.load(toolchain)["toolchain"]["channel"]
    except (OSError, tomllib.TOMLDecodeError, KeyError) as error:
        detail = f"cannot read the pinned toolchain from rust-toolchain.toml: {error}"
        raise RuntimeError(detail) from error


def doc_targets(metadata: dict) -> list[DocTarget]:
    """Derive library and binary targets for every workspace member."""
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
    """Map one Cargo target to its measurable target, if any."""
    kinds: list[str] = target.get("kind", [])
    if "lib" in kinds:
        return [DocTarget(package["name"], "lib", None)]
    if "bin" in kinds:
        return [DocTarget(package["name"], "bin", target["name"])]
    return []


def rustdoc_args(target: DocTarget, toolchain: str) -> list[str]:
    """Build Cargo's Rustdoc coverage command for one target."""
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
    """Decode and aggregate one generated Rustdoc coverage payload."""
    try:
        per_file = json.loads(output)
    except json.JSONDecodeError as error:
        raise coverage_json_error(target, str(error)) from error
    try:
        return aggregate_coverage_payload(per_file)
    except (KeyError, TypeError, ValueError, OverflowError) as error:
        detail = str(error)
        if detail != "expected an object":
            detail = f"each entry requires total and with_docs: {error}"
        raise coverage_json_error(target, detail) from error


def coverage_json_error(target: DocTarget, detail: str) -> RuntimeError:
    """Build a measurement error naming its target and coverage-output detail."""
    message = (
        f"cargo rustdoc for {target.package} {target.kind}"
        f" ({target.name or 'lib'}) did not emit coverage JSON: {detail}"
    )
    return RuntimeError(message)


def coverage_output_path(
    target: DocTarget, output: str, manifest_root: pathlib.Path
) -> pathlib.Path:
    """Return the generated coverage JSON path reported by Rustdoc."""
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
    """Run Rustdoc coverage for one target and sum its per-file counts."""
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
    output_path = coverage_output_path(target, result.stdout, manifest_root)
    try:
        output = output_path.read_text(encoding="utf-8")
    except OSError as error:
        detail = f"cannot read generated coverage JSON at {output_path}: {error}"
        raise coverage_json_error(target, detail) from error
    return parse_coverage_output(target, output)


def load_metadata(toolchain: str, manifest_root: pathlib.Path) -> dict:
    """Return Cargo metadata for the workspace rooted at ``manifest_root``."""
    args = [
        cargo_executable(),
        f"+{toolchain}",
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
    ]
    try:
        result = subprocess.run(  # noqa: S603
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


def run_measurements(
    toolchain: str, manifest_root: pathlib.Path
) -> tuple[Coverage, list[tuple[DocTarget, Coverage]]]:
    """Measure every target and return aggregate plus target-specific coverage."""
    totals = Coverage(0, 0)
    rows: list[tuple[DocTarget, Coverage]] = []
    for target in doc_targets(load_metadata(toolchain, manifest_root)):
        coverage = measure(target, toolchain, manifest_root)
        rows.append((target, coverage))
        totals += coverage
    return totals, rows
