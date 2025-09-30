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

    Notes
    -----
    Reads environment variables provided by GitHub Actions and raises a
    configuration error when required values are absent:

    - ``GITHUB_OUTPUT``: Required path receiving workflow outputs.
    - ``GITHUB_WORKSPACE``: Optional checkout root. Defaults to ``Path('.')``
      when absent.
    - ``BIN_EXT``: Optional binary suffix override for non-standard bundles.
    """
    github_output_env = os.environ.get("GITHUB_OUTPUT")
    if not github_output_env:
        print(
            "::error title=Configuration Error::",
            "GITHUB_OUTPUT environment variable is missing",
            file=sys.stderr,
        )
        raise SystemExit(1)

    workspace = Path(os.environ.get("GITHUB_WORKSPACE", "."))
    bin_ext = os.environ.get("BIN_EXT", "")

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
