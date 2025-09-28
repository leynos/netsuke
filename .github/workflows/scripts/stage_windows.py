# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "cyclopts>=3.24.0,<4.0.0",
# ]
# ///

"""Stage Windows release artefacts for GitHub workflows."""

from __future__ import annotations

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
    github_output: typ.Annotated[
        Path,
        Parameter(
            env_var="GITHUB_OUTPUT",
            converter=Path,
            required=True,
        ),
    ],
    workspace: typ.Annotated[
        Path, Parameter(env_var="GITHUB_WORKSPACE", converter=Path)
    ] = Path(),
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
    github_output : Path
        File that receives workflow output key-value pairs.
    workspace : Path, optional
        Workspace root to resolve build outputs when staging artefacts.
    """
    config = StagingConfig(
        bin_name=bin_name,
        target=target,
        platform=platform,
        arch=arch,
        workspace=workspace,
        bin_ext=".exe",
    )

    try:
        stage_artifacts(config, github_output)
    except RuntimeError as exc:
        print(f"::error title=Packaging failure::{exc}")
        raise SystemExit(1) from exc


if __name__ == "__main__":
    app()
