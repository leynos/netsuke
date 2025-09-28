# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "cyclopts>=3.24.0,<4.0.0",
# ]
# ///

"""Stage Windows release artefacts for GitHub workflows."""

from __future__ import annotations

import os
import typing as typ
from pathlib import Path

from cyclopts import App, Parameter
from stage_common import StagingConfig, stage_artifacts

app = App()


@app.default
def stage_windows(
    bin_name: typ.Annotated[str, Parameter(env_var="BIN_NAME")],
    target: typ.Annotated[str, Parameter(env_var="TARGET")],
    platform: typ.Annotated[str, Parameter(env_var="PLATFORM")],
    arch: typ.Annotated[str, Parameter(env_var="ARCH")],
) -> None:
    """Stage Windows artefacts and expose their paths via workflow outputs.

    Parameters
    ----------
    bin_name : str
        Name of the compiled binary to collect.
    target : str
        Rust compilation target triple identifying the build output.
    platform : str
        Display label for the operating system flavour.
    arch : str
        CPU architecture string for packaging (for example ``"x86_64"``).

    Notes
    -----
    The ``GITHUB_OUTPUT`` environment variable must point to the workflow output
    file. ``GITHUB_WORKSPACE`` may optionally redefine the workspace root when
    locating artefacts.
    """
    github_output_env = os.environ.get("GITHUB_OUTPUT")
    if github_output_env is None:
        print(
            "::error title=Configuration Error::"
            "GITHUB_OUTPUT environment variable is required."
        )
        raise SystemExit(1)

    workspace_env = os.environ.get("GITHUB_WORKSPACE", ".")

    config = StagingConfig(
        bin_name=bin_name,
        target=target,
        platform=platform,
        arch=arch,
        workspace=Path(workspace_env),
        bin_ext=".exe",
    )

    try:
        stage_artifacts(config, Path(github_output_env))
    except RuntimeError as exc:
        print(f"::error title=Packaging failure::{exc}")
        raise SystemExit(1) from exc


if __name__ == "__main__":
    app()
