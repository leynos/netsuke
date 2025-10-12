"""Normalise Windows paths from stage action outputs.

This script reads the ``BINARY_PATH`` and ``LICENSE_PATH`` environment
variables emitted by the Stage composite action, validates that both are
present, and writes normalised Windows paths to ``GITHUB_OUTPUT`` for
downstream workflow steps.

Usage
-----
Invoke from a GitHub Actions workflow step::

    - name: Normalise Windows paths
      shell: bash
      env:
        BINARY_PATH: ${{ steps.stage.outputs.binary_path }}
        LICENSE_PATH: ${{ steps.stage.outputs.license_path }}
      run: python .github/workflows/scripts/normalise_windows_paths.py
"""

from __future__ import annotations

import os
import sys
from pathlib import PureWindowsPath


def main() -> int:
    """Read stage outputs and write normalised paths to ``GITHUB_OUTPUT``.

    Returns
    -------
    int
        Exit code: 0 on success, 1 when environment variables are missing,
        empty, or when ``GITHUB_OUTPUT`` is unset.
    """
    try:
        binary = os.environ["BINARY_PATH"]
    except KeyError as exc:  # pragma: no cover - exercised in workflow runtime
        print(
            f"::error title=Stage output missing::Missing env {exc}",
            file=sys.stderr,
        )
        return 1
    try:
        licence = os.environ["LICENSE_PATH"]
    except KeyError as exc:  # pragma: no cover - exercised in workflow runtime
        print(
            f"::error title=Stage output missing::Missing env {exc}",
            file=sys.stderr,
        )
        return 1

    if not binary:
        print(
            "::error title=Stage output missing::BINARY_PATH output empty",
            file=sys.stderr,
        )
        return 1
    if not licence:
        print(
            "::error title=Stage output missing::LICENSE_PATH output empty",
            file=sys.stderr,
        )
        return 1

    github_output = os.environ.get("GITHUB_OUTPUT")
    if not github_output:
        print(
            "::error title=Missing GITHUB_OUTPUT::Environment variable unset",
            file=sys.stderr,
        )
        return 1

    binary_path = PureWindowsPath(binary)
    license_path = PureWindowsPath(licence)

    with open(github_output, "a", encoding="utf-8") as handle:
        handle.write(f"binary_path={binary_path}\n")
        handle.write(f"license_path={license_path}\n")
    return 0


if __name__ == "__main__":  # pragma: no cover - invoked by GitHub Actions
    raise SystemExit(main())
