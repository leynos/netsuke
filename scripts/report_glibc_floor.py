#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts>=4.25.3,<5", "cuprum>=0.1.0,<0.2.0"]
# ///
"""Report a Linux release binary's highest required GLIBC version."""

import pathlib
import re
import sys
import typing as typ

from cuprum import CommandResult, Program, ProgramCatalogue, sh
from cuprum.catalogue import ProjectSettings

import cyclopts
from cyclopts import App, Parameter

READELF_CATALOGUE = ProgramCatalogue(
    projects=(
        ProjectSettings(
            name="report-glibc-floor",
            programs=(Program("readelf"),),
            documentation_locations=(),
            noise_rules=(),
        ),
    )
)
app = App(config=cyclopts.config.Env("INPUT_", command=False))


def _version_needs_section(output: str) -> str:
    r"""Return only the readelf version-needs section.

    Returns
    -------
    str
        The needs section, or an empty string if the output has none.

    Examples
    --------
    >>> output = "Version needs section x"
    >>> output += "\nGLIBC_2.9\nVersion symbols section y"
    >>> _version_needs_section(output)
    'Version needs section x\nGLIBC_2.9\n'
    """
    match = re.search(
        r"^Version needs section.*?(?=^Version [a-z]+ section|\Z)",
        output,
        flags=re.MULTILINE | re.DOTALL,
    )
    return match.group(0) if match else ""


def _highest_glibc_version(output: str) -> str:
    r"""Return the greatest GLIBC version required by readelf output.

    Returns
    -------
    str
        The greatest required GLIBC version, or an empty string if absent.

    Examples
    --------
    >>> output = "Version needs section x"
    >>> output += "\nGLIBC_2.9 GLIBC_2.34\nVersion symbols section y\nGLIBC_2.99"
    >>> _highest_glibc_version(output)
    'GLIBC_2.34'
    """
    section = _version_needs_section(output)
    versions = re.findall(r"GLIBC_([0-9]+(?:\.[0-9]+)*)", section)
    if not versions:
        return ""

    highest = max(versions, key=lambda version: tuple(map(int, version.split("."))))
    return f"GLIBC_{highest}"


def _readelf(binary: pathlib.Path) -> CommandResult:
    """Read version requirements from one binary without invoking a shell.

    Returns
    -------
    CommandResult
        Captured ``readelf`` output and exit status.

    Examples
    --------
    ``_readelf(pathlib.Path("target/example/release/netsuke"))`` passes the
    full path as one argument after ``--version-info``.
    """
    command = sh.make(Program("readelf"), catalogue=READELF_CATALOGUE)(
        "--version-info", str(binary)
    )
    return command.run_sync()


@app.default
def report_glibc_floor(
    *,
    target: typ.Annotated[str, Parameter(required=True)],
    bin_name: typ.Annotated[str, Parameter(required=True)],
    summary_path: typ.Annotated[
        pathlib.Path, Parameter(env_var="GITHUB_STEP_SUMMARY", required=True)
    ],
) -> None:
    """Print and append the release binary's portability floor.

    Raises
    ------
    SystemExit
        If ``readelf`` fails, with its exit status.

    Examples
    --------
    With ``INPUT_TARGET=x86_64-unknown-linux-gnu`` and ``INPUT_BIN_NAME=netsuke``,
    inspect the target's release binary and report its highest needed GLIBC.
    """
    binary = pathlib.Path("target") / target / "release" / bin_name
    result = _readelf(binary)
    if result.stderr:
        sys.stderr.write(result.stderr)
    if result.exit_code != 0:
        raise SystemExit(result.exit_code)

    floor = _highest_glibc_version(result.stdout or "")
    if not floor:
        raise SystemExit(1)

    print(f"glibc floor for {target}: {floor}")
    with summary_path.open("a", encoding="utf-8") as summary:
        summary.write(f"- glibc floor for `{target}`: `{floor}`\n")


if __name__ == "__main__":
    app()
