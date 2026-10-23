#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts>=4.25.3,<5"]
# ///
"""Resolve and export the WiX extension version supplied by the workflow."""

import pathlib
import sys
import typing as typ

import cyclopts
from cyclopts import App, Parameter

app = App(config=cyclopts.config.Env("INPUT_", command=False))


class ExtensionVersionError(ValueError):
    """Represent an invalid WiX extension version supplied by a workflow."""


class EmptyExtensionVersionError(ExtensionVersionError):
    """Report a WiX extension version that is empty."""

    def __init__(self) -> None:
        """Initialize the error with the empty-version diagnostic."""
        super().__init__("WIX_EXTENSION_VERSION must be set")


class LineBreakExtensionVersionError(ExtensionVersionError):
    """Report a WiX extension version that contains a line break."""

    def __init__(self) -> None:
        """Initialize the error with the line-break diagnostic."""
        super().__init__("WIX_EXTENSION_VERSION must not contain line breaks")


def resolve_extension_version(wix_extension_version: str) -> str:
    """Validate and return the requested extension version.

    >>> resolve_extension_version("8.1.2")
    '8.1.2'

    Returns
    -------
    str
        The unchanged requested extension version.

    Raises
    ------
    EmptyExtensionVersionError
        If the version is empty.
    LineBreakExtensionVersionError
        If the version contains a carriage return or line feed.
    """
    if not wix_extension_version:
        raise EmptyExtensionVersionError
    if "\n" in wix_extension_version or "\r" in wix_extension_version:
        raise LineBreakExtensionVersionError
    return wix_extension_version


@app.default
def export_extension_version(
    *,
    wix_extension_version: typ.Annotated[str, Parameter(required=True)],
    output_path: typ.Annotated[str, Parameter(env_var="GITHUB_OUTPUT", required=True)],
) -> None:
    """Validate the version, append it to the workflow output, and report it."""
    try:
        version = resolve_extension_version(wix_extension_version)
    except ExtensionVersionError as error:
        print(error, file=sys.stderr)
        raise SystemExit(1) from error

    with pathlib.Path(output_path).open("a", encoding="utf-8") as output:
        output.write(f"value={version}\n")
    print(f"Resolved WiX extension version: {version}")


if __name__ == "__main__":
    app()
