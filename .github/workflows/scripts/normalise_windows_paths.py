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

SCRIPTS_DIR = Path(__file__).resolve().parent
MODULE_DIR = SCRIPTS_DIR.parent.parent / "actions" / "stage" / "scripts"
if str(MODULE_DIR) not in sys.path:
    sys.path.insert(0, str(MODULE_DIR))

from stage_common.errors import (  # Imported after sys.path mutation.
    StageError,
)


def _validate_artefact_paths(mapping: dict[str, str]) -> tuple[str, str]:
    """Extract and validate ``binary_path`` and ``license_path`` from the mapping.

    Confirms that both ``binary_path`` and ``license_path`` keys are present
    and that their values are non-empty strings.

    Args:
        mapping: Dictionary containing artefact paths.

    Returns:
        Tuple of (binary_path, license_path) values.

    Raises:
        StageError: If either key is missing or has an empty value.
    """

    try:
        binary = mapping["binary_path"]
        licence = mapping["license_path"]
    except KeyError as exc:  # pragma: no cover - exercised in workflow runtime
        message = f"::error title=Stage output missing::Missing artefact {exc}"
        raise StageError(message) from exc

    if not binary:
        message = "::error title=Stage output missing::binary_path output empty"
        raise StageError(message)
    if not licence:
        message = "::error title=Stage output missing::license_path output empty"
        raise StageError(message)
    return binary, licence


def main() -> int:
    """Read ``ARTEFACT_MAP`` and write normalised paths to ``GITHUB_OUTPUT``."""
    try:
        mapping = json.loads(os.environ["ARTEFACT_MAP"])
    except KeyError as exc:  # pragma: no cover
        print(
            f"::error title=Stage output missing::Missing env {exc}",
            file=sys.stderr,
        )
        return 1
    except json.JSONDecodeError as exc:  # pragma: no cover
        print(
            f"::error title=Invalid ARTEFACT_MAP::Failed to parse JSON: {exc}",
            file=sys.stderr,
        )
        return 1

    try:
        binary, licence = _validate_artefact_paths(mapping)
    except StageError as exc:
        print(str(exc), file=sys.stderr)
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

    # Verify paths are not just whitespace after normalisation
    if not str(binary_path).strip():
        print(
            "::error title=Invalid path::binary_path normalised to empty",
            file=sys.stderr,
        )
        return 1
    if not str(license_path).strip():
        print(
            "::error title=Invalid path::license_path normalised to empty",
            file=sys.stderr,
        )
        return 1

    with Path(github_output).open("a", encoding="utf-8") as handle:
        handle.write(f"binary_path={binary_path}\n")
        handle.write(f"license_path={license_path}\n")
    return 0


if __name__ == "__main__":  # pragma: no cover - invoked by GitHub Actions
    raise SystemExit(main())
