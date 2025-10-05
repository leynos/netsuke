#!/usr/bin/env python3
"""Utility helpers for extracting fields from Cargo.toml."""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

import tomllib

PARSER_DESCRIPTION = " ".join(
    [
        "Read selected fields from a Cargo.toml manifest and print them to",
        "stdout.",
    ]
)


def parse_args() -> argparse.Namespace:
    """Return the parsed CLI arguments for manifest field extraction."""
    parser = argparse.ArgumentParser(description=PARSER_DESCRIPTION)
    parser.add_argument(
        "field", choices=("name", "version"), help="The manifest field to print."
    )
    parser.add_argument(
        "--manifest-path",
        default=None,
        help=(
            "Path to the Cargo.toml file. Defaults to the CARGO_TOML_PATH "
            "environment variable when set, otherwise Cargo.toml in the "
            "current working directory."
        ),
    )
    return parser.parse_args()


def read_manifest(path: Path) -> dict[str, object]:
    """
    Load and return the parsed Cargo manifest as a dictionary.

    Parameters
    ----------
    path : Path
        Path to the ``Cargo.toml`` file.

    Returns
    -------
    dict[str, object]
        Parsed manifest fields keyed by section.

    Raises
    ------
    FileNotFoundError
        If the manifest file does not exist.
    tomllib.TOMLDecodeError
        If the manifest contains invalid TOML syntax.

    Examples
    --------
    >>> from pathlib import Path
    >>> manifest_path = Path("Cargo.toml")
    >>> data = read_manifest(manifest_path)
    >>> "package" in data
    True
    """
    if not path.is_file():
        message = f"Manifest {path} does not exist"
        raise FileNotFoundError(message)
    with path.open("rb") as handle:
        return tomllib.load(handle)


def get_field(manifest: dict[str, object], field: str) -> str:
    """
    Extract a package field from the manifest, raising if it is missing.

    Parameters
    ----------
    manifest : dict[str, object]
        The parsed Cargo manifest dictionary.
    field : str
        The package field to extract, such as ``"name"`` or ``"version"``.

    Returns
    -------
    str
        The non-empty field value from the package section.

    Raises
    ------
    KeyError
        If the package table is missing or the field is absent or blank.

    Examples
    --------
    >>> manifest = {"package": {"name": "netsuke", "version": "1.2.3"}}
    >>> get_field(manifest, "name")
    'netsuke'
    """
    package = manifest.get("package") or {}
    if not isinstance(package, dict):
        message = "package table missing from manifest"
        raise KeyError(message)
    value = package.get(field, "")
    if not isinstance(value, str) or not value:
        message = f"package.{field} is missing"
        raise KeyError(message)
    return value


def main() -> int:
    """Entry point for the manifest reader CLI."""
    args = parse_args()
    manifest_path = args.manifest_path or os.environ.get(
        "CARGO_TOML_PATH", "Cargo.toml"
    )
    try:
        manifest = read_manifest(Path(manifest_path))
        value = get_field(manifest, args.field)
    except (KeyError, FileNotFoundError, tomllib.TOMLDecodeError) as exc:
        print(exc, file=sys.stderr)
        return 1
    print(value, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
