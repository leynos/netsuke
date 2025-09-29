# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "cyclopts>=3.24.0,<4.0.0",
# ]
# ///
"""Stage macOS release artefacts using Cyclopts parameters.

Examples
--------
>>> import os
>>> from pathlib import Path
>>> os.environ.update(
...     {
...         "BIN_NAME": "netsuke",
...         "TARGET": "aarch64-apple-darwin",
...         "PLATFORM": "macos",
...         "ARCH": "arm64",
...         "GITHUB_OUTPUT": str(Path("/tmp") / "out"),
...     }
... )
>>> stage()  # doctest: +SKIP
"""

from __future__ import annotations

import os
import sys
import typing as typ
from pathlib import Path

from cyclopts import App, Parameter
from stage_common import StagingConfig, stage_artifacts

app = App()


@app.default
def stage(
    bin_name: typ.Annotated[str, Parameter(env_var="BIN_NAME")],
    target: typ.Annotated[str, Parameter(env_var="TARGET")],
    platform: typ.Annotated[str, Parameter(env_var="PLATFORM")],
    arch: typ.Annotated[str, Parameter(env_var="ARCH")],
    workspace: typ.Annotated[
        Path, Parameter(env_var="GITHUB_WORKSPACE", converter=Path)
    ] = Path(),
    bin_ext: typ.Annotated[str, Parameter(env_var="BIN_EXT")] = "",
) -> None:
    """Stage macOS artefacts and emit GitHub Actions outputs.

    Parameters
    ----------
    bin_name : str
        Name of the compiled binary to collect.
    target : str
        Rust compilation target triple identifying the build output.
    platform : str
        Display label for the operating system flavour.
    arch : str
        CPU architecture string for packaging (for example ``"arm64"``).
    workspace : Path, optional
        GitHub workspace directory. Defaults to ``Path()`` when the
        ``GITHUB_WORKSPACE`` variable is not provided.
    bin_ext : str, optional
        Optional binary suffix override. macOS artefacts typically omit this
        value, but the parameter remains for parity with other platforms.

    Notes
    -----
    Reads the ``GITHUB_OUTPUT`` environment variable provided by GitHub Actions
    and raises a configuration error when it is absent.
    """
    github_output_env = os.environ.get("GITHUB_OUTPUT")
    if not github_output_env:
        print(
            "::error title=Configuration Error::"
            "GITHUB_OUTPUT environment variable is missing",
            file=sys.stderr,
        )
        raise SystemExit(1)

    config = StagingConfig(
        bin_name=bin_name,
        target=target,
        platform=platform,
        arch=arch,
        workspace=workspace,
        bin_ext=bin_ext,
    )

    try:
        stage_artifacts(config, Path(github_output_env))
    except RuntimeError as exc:
        print(f"::error title=Packaging failure::{exc}", file=sys.stderr)
        raise SystemExit(1) from exc


if __name__ == "__main__":
    app()
