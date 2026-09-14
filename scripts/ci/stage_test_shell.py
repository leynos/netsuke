#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts==4.25.2", "cuprum @ git+https://github.com/leynos/cuprum@a2134c7a3966b224eaed917efb94f8090ce5104a"]
# ///
"""Stage GNU Awk as a regular ``awk`` executable for the test sandbox.

The hermetic dev-fast sandbox probes ``awk`` through a capability-backed
directory handle, which deliberately cannot follow a symlink that escapes its
containing directory. Ubuntu exposes ``awk`` as exactly such an alternatives
symlink, so this script installs the ``gawk`` package, copies its binary into
``$RUNNER_TEMP/netsuke-test-bin/awk`` as a regular file, publishes that
directory through ``GITHUB_PATH``, and proves the staged copy runs.

Parameters arrive from the environment: ``RUNNER_TEMP`` and ``GITHUB_PATH``
are ambient in GitHub Actions; ``INPUT_PACKAGE`` and ``INPUT_STAGING_NAME``
override the defaults.
"""

import pathlib
import shutil
import sys
import typing as typ

from ci_support import (
    CiScriptError,
    append_github_path,
    catalogue,
    describe_failure,
    run,
)

import cyclopts
from cyclopts import App, Parameter

SUDO = "sudo"
APT_GET = "apt-get"
PRIVILEGED = catalogue(SUDO)

app = App(config=cyclopts.config.Env("INPUT_", command=False))


def install_package(package: str) -> None:
    """Install ``package`` through ``sudo apt-get`` without recommends."""
    for arguments in (
        ("update",),
        ("install", "--yes", "--no-install-recommends", package),
    ):
        result = run(SUDO, APT_GET, *arguments, allowed=PRIVILEGED)
        if not result.ok:
            message = f"installing {package} failed: {describe_failure(result)}"
            raise CiScriptError(message)


def locate_binary(package: str) -> pathlib.Path:
    """Return the installed awk implementation ``package`` put on PATH.

    Returns
    -------
    pathlib.Path
        The executable ``shutil.which`` resolved for ``package``.

    Raises
    ------
    CiScriptError
        If no executable named ``package`` is on PATH after installation.
    """
    source = shutil.which(package)
    if source is None:
        message = f"{package} is not on PATH after installation"
        raise CiScriptError(message)
    return pathlib.Path(source)


def stage_copy(source: pathlib.Path, staging_dir: pathlib.Path) -> pathlib.Path:
    """Copy ``source`` into ``staging_dir`` as a regular executable ``awk``.

    ``shutil.copyfile`` follows symlinks and writes a new regular file, which
    is the property the sandbox probe depends on.

    Returns
    -------
    pathlib.Path
        The staged ``awk`` executable.
    """
    staging_dir.mkdir(parents=True, exist_ok=True)
    staged = staging_dir / "awk"
    shutil.copyfile(source, staged)
    staged.chmod(0o755)
    return staged


def verify_staged(staged: pathlib.Path) -> str:
    """Run the staged ``awk --version`` and return its first output line."""
    result = run(str(staged), "--version", allowed=catalogue(str(staged)))
    if not result.ok:
        message = f"the staged awk does not run: {describe_failure(result)}"
        raise CiScriptError(message)
    return (result.stdout or "").strip().splitlines()[0] if result.stdout else ""


@app.default
def main(
    *,
    runner_temp: typ.Annotated[pathlib.Path, Parameter(env_var="RUNNER_TEMP")],
    github_path: typ.Annotated[pathlib.Path, Parameter(env_var="GITHUB_PATH")],
    package: str = "gawk",
    staging_name: str = "netsuke-test-bin",
) -> int:
    """Install the awk package and stage its binary for the sandbox.

    Parameters
    ----------
    runner_temp
        The runner's scratch directory; the staging directory lives under it.
    github_path
        The ``GITHUB_PATH`` file that publishes the staging directory.
    package
        The apt package providing the awk implementation.
    staging_name
        The staging directory's name under ``runner_temp``.

    Returns
    -------
    int
        ``0`` on success, ``1`` with a message on standard error otherwise.
    """
    try:
        install_package(package)
        source = locate_binary(package)
        staged = stage_copy(source, runner_temp / staging_name)
        append_github_path(staged.parent, github_path)
        print(f"staged {source} as {staged}: {verify_staged(staged)}")
    except CiScriptError as error:
        print(f"stage_test_shell: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    app()
