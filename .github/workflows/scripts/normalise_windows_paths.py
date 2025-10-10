"""Normalise Windows paths from stage action outputs.

This script reads the ``ARTEFACT_MAP`` environment variable (a JSON artefact
map emitted by the Stage composite action), validates that ``binary_path`` and
``license_path`` are present, and writes normalised Windows paths to
``GITHUB_OUTPUT`` for downstream workflow steps.

Usage
-----
Invoke from a GitHub Actions workflow step::

    - name: Normalise Windows paths
      shell: bash
      env:
        ARTEFACT_MAP: ${{ steps.stage.outputs.artefact_map }}
      run: python .github/workflows/scripts/normalise_windows_paths.py
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path, PureWindowsPath


def main() -> int:
    """Read ``ARTEFACT_MAP`` and write normalised paths to ``GITHUB_OUTPUT``."""
    try:
        mapping = json.loads(os.environ["ARTEFACT_MAP"])
    except KeyError as exc:  # pragma: no cover - exercised in workflow runtime
        print(
            f"::error title=Stage output missing::Missing env {exc}",
            file=sys.stderr,
        )
        return 1

    try:
        binary = mapping["binary_path"]
        licence = mapping["license_path"]
    except KeyError as exc:  # pragma: no cover - exercised in workflow runtime
        print(
            f"::error title=Stage output missing::Missing artefact {exc}",
            file=sys.stderr,
        )
        return 1

    if not binary:
        print(
            "::error title=Stage output missing::binary_path output empty",
            file=sys.stderr,
        )
        return 1
    if not licence:
        print(
            "::error title=Stage output missing::license_path output empty",
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

    with Path(github_output).open("a", encoding="utf-8") as handle:
        handle.write(f"binary_path={binary_path}\n")
        handle.write(f"license_path={license_path}\n")
    return 0


if __name__ == "__main__":  # pragma: no cover - invoked by GitHub Actions
    raise SystemExit(main())
