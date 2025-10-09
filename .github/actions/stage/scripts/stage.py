# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "cyclopts>=0.14",
# ]
# ///

"""Command-line entry point for the staging helper.

Examples
--------
Run the staging helper locally after exporting the required environment
variables::

    export GITHUB_WORKSPACE="$(pwd)"
    export GITHUB_OUTPUT="$(mktemp)"
    uv run .github/actions/stage/scripts/stage.py \
        .github/release-staging.toml linux-x86_64
"""

from __future__ import annotations

import sys
from pathlib import Path

from stage_common import StageError, load_config, require_env_path, stage_artefacts

import cyclopts

app = cyclopts.App(help="Stage release artefacts using a TOML configuration file.")


@app.default
def main(config_file: Path, target: str) -> None:
    """Stage artefacts for ``target`` using ``config_file``.

    Parameters
    ----------
    config_file:
        Path to the project-specific TOML configuration file.
    target:
        Target key in the configuration file (for example ``"linux-x86_64"``).
    """
    try:
        config_path = Path(config_file)
        github_output = require_env_path("GITHUB_OUTPUT")
        config = load_config(config_path, target)
        result = stage_artefacts(config, github_output)
    except (FileNotFoundError, StageError) as exc:
        print(f"::error title=Staging Failure::{exc}", file=sys.stderr)
        raise SystemExit(1) from exc

    staged_rel = result.staging_dir.relative_to(config.workspace)
    print(
        f"Staged {len(result.staged_artefacts)} artefact(s) into '{staged_rel}'.",
        file=sys.stderr,
    )


if __name__ == "__main__":
    app()
