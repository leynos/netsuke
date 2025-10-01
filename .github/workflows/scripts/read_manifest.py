#!/usr/bin/env python3
"""Utility helpers for extracting fields from Cargo.toml."""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

import tomllib


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Read selected fields from a Cargo.toml manifest and print them to "
            "stdout."
        )
    )
    parser.add_argument(
        "field",
        choices=("name", "version"),
        help="The manifest field to print."
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
    if not path.is_file():
        raise FileNotFoundError(f"Manifest {path} does not exist")
    with path.open("rb") as handle:
        return tomllib.load(handle)


def get_field(manifest: dict[str, object], field: str) -> str:
    package = manifest.get("package") or {}
    if not isinstance(package, dict):
        raise KeyError("package table missing from manifest")
    value = package.get(field, "")
    if not isinstance(value, str) or not value:
        raise KeyError(f"package.{field} is missing")
    return value


def main() -> int:
    args = parse_args()
    manifest_path = args.manifest_path or os.environ.get(
        "CARGO_TOML_PATH", "Cargo.toml"
    )
    try:
        manifest = read_manifest(Path(manifest_path))
        value = get_field(manifest, args.field)
    except (KeyError, FileNotFoundError) as exc:
        print(exc, file=sys.stderr)
        return 1
    print(value, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
