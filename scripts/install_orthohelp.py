#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts>=4.25.3,<5", "cuprum>=0.1.0,<0.2.0"]
# ///
"""Install the pinned prebuilt cargo-orthohelp release when the cache misses."""

import re
import typing as typ

from cuprum import CommandResult, Program, ProgramCatalogue, sh
from cuprum.catalogue import ProjectSettings

import cyclopts
from cyclopts import App, Parameter

INSTALLER_CATALOGUE = ProgramCatalogue(
    projects=(
        ProjectSettings(
            name="install-orthohelp",
            programs=(Program("cargo-orthohelp"), Program("cargo")),
            documentation_locations=(),
            noise_rules=(),
        ),
    )
)
app = App(config=cyclopts.config.Env("INPUT_", command=False))


def _run(program: str, *arguments: str, echo: bool = False) -> CommandResult:
    """Run one program from the installer allowlist.

    Returns
    -------
    CommandResult
        The captured output and exit status from the allowlisted program.

    Examples
    --------
    ``_run("cargo-orthohelp", "--version")`` probes the cached tool.
    """
    command = sh.make(Program(program), catalogue=INSTALLER_CATALOGUE)(*arguments)
    return command.run_sync(echo=echo)


def _has_expected_version(output: str, version: str) -> bool:
    r"""Match the requested version as a complete whitespace-delimited token.

    Returns
    -------
    bool
        Whether the output contains the requested version as one token.

    Examples
    --------
    >>> _has_expected_version("cargo-orthohelp 0.9.1\n", "0.9.1")
    True
    >>> _has_expected_version("cargo-orthohelp 0.9.10\n", "0.9.1")
    False
    """
    return re.search(rf"(^|\s){re.escape(version)}(\s|$)", output) is not None


def _has_cached_version(installed: CommandResult | None, version: str) -> bool:
    r"""Report whether the optional probe found the requested cached version.

    Keep this classifier within the installer: it composes the optional
    ``cargo-orthohelp --version`` result with the version matcher and is called
    only by the CLI entry point. It does not execute the installation fallback.

    Returns
    -------
    bool
        Whether the probe completed successfully and found the requested version.

    Examples
    --------
    >>> _has_cached_version(None, "0.9.1")
    False
    >>> probe = CommandResult(
    ...     Program("cargo-orthohelp"), ("cargo-orthohelp", "--version"),
    ...     0, 42, "cargo-orthohelp 0.9.1" + chr(10), "",
    ... )
    >>> _has_cached_version(probe, "0.9.1")
    True
    """
    if installed is None:
        return False
    if installed.exit_code != 0:
        return False
    return _has_expected_version(installed.stdout or "", version)


@app.default
def install_orthohelp(*, version: typ.Annotated[str, Parameter(required=True)]) -> None:
    """Install the requested prebuilt tool only when the cache lacks it.

    Raises
    ------
    SystemExit
        If the prebuilt installation fails, with ``cargo binstall``'s status.

    Examples
    --------
    With ``INPUT_VERSION=0.9.1``, keep a matching cached binary or run
    ``cargo binstall --no-confirm --locked --disable-strategies compile``.
    """
    try:
        installed = _run("cargo-orthohelp", "--version")
    except FileNotFoundError:
        # A cache miss is expected on fresh runners, where the binary is absent.
        installed = None
    if _has_cached_version(installed, version):
        print(f"cargo-orthohelp {version} restored from the cache")
        return

    installation = _run(
        "cargo",
        "binstall",
        "--no-confirm",
        "--locked",
        "--disable-strategies",
        "compile",
        f"cargo-orthohelp@{version}",
        echo=True,
    )
    if installation.exit_code != 0:
        raise SystemExit(installation.exit_code)


if __name__ == "__main__":
    app()
