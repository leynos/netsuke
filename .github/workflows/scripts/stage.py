# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "cyclopts>=3.24.0,<4.0.0",
# ]
# ///

"""Stage release artefacts for GitHub workflows.

Examples
--------
Run within a GitHub Actions step:

>>> import os
>>> from pathlib import Path
>>> os.environ.update(
...     {
...         "BIN_NAME": "netsuke",
...         "TARGET": "x86_64-unknown-linux-gnu",
...         "PLATFORM": "linux",
...         "ARCH": "amd64",
...         "GITHUB_OUTPUT": str(Path("/tmp") / "out"),
...         "GITHUB_WORKSPACE": ".",
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


def _resolve_output_path() -> Path:
    """Return the GitHub output file path or terminate early.

    Examples
    --------
    >>> import os
    >>> os.environ.pop("GITHUB_OUTPUT", None)
    >>> try:
    ...     _ = _resolve_output_path()
    ... except SystemExit:
    ...     print("halted")
    halted
    """
    github_output_env = os.environ.get("GITHUB_OUTPUT")
    if github_output_env is None:
        print(
            "::error title=Configuration Error::",
            "GITHUB_OUTPUT environment variable is required.",
            file=sys.stderr,
        )
        raise SystemExit(1)
    return Path(github_output_env)


def _infer_bin_ext(platform: str) -> str:
    """Infer the binary extension for the provided platform string.

    Parameters
    ----------
    platform : str
        Value from the ``PLATFORM`` environment variable.

    Returns
    -------
    str
        ``".exe"`` for Windows platforms, otherwise an empty string.

    Examples
    --------
    >>> _infer_bin_ext("windows")
    '.exe'
    >>> _infer_bin_ext("linux")
    ''
    """
    if platform.lower().startswith("win"):
        return ".exe"
    return ""


@app.default
def stage(
    bin_name: typ.Annotated[str, Parameter(env_var="BIN_NAME")],
    target: typ.Annotated[str, Parameter(env_var="TARGET")],
    platform: typ.Annotated[str, Parameter(env_var="PLATFORM")],
    arch: typ.Annotated[str, Parameter(env_var="ARCH")],
) -> None:
    """Stage artefacts and expose their paths via workflow outputs.

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
    ``GITHUB_OUTPUT`` must point to the workflow output file. Optionally set
    ``GITHUB_WORKSPACE`` to override the working directory or ``BIN_EXT`` to
    force an extension.

    Examples
    --------
    >>> import os
    >>> from pathlib import Path
    >>> os.environ.update(
    ...     {
    ...         "BIN_NAME": "netsuke",
    ...         "TARGET": "x86_64-unknown-linux-gnu",
    ...         "PLATFORM": "linux",
    ...         "ARCH": "amd64",
    ...         "GITHUB_OUTPUT": str(Path("/tmp") / "out"),
    ...     }
    ... )
    >>> stage()  # doctest: +SKIP
    """
    github_output = _resolve_output_path()
    workspace_env = os.environ.get("GITHUB_WORKSPACE", ".")
    bin_ext = os.environ.get("BIN_EXT") or _infer_bin_ext(platform)

    config = StagingConfig(
        bin_name=bin_name,
        target=target,
        platform=platform,
        arch=arch,
        workspace=Path(workspace_env),
        bin_ext=bin_ext,
    )

    try:
        stage_artifacts(config, github_output)
    except RuntimeError as exc:
        print(f"::error title=Packaging failure::{exc}", file=sys.stderr)
        raise SystemExit(1) from exc


if __name__ == "__main__":
    app()
