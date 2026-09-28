#!/usr/bin/env python3
"""Read and validate package metadata from a Cargo manifest.

The release workflow and Linux package validator share this reader so package
metadata has one normalization and validation boundary.
"""

import argparse
import enum
import os
import pathlib
import sys
import tomllib
import typing as typ
import unicodedata


class PackageMetadataIssue(enum.Enum):
    """Name one invalid or unreadable Cargo package metadata condition.

    Attributes
    ----------
    FIELD_NOT_STRING
        A required value is not a string.
    FIELD_CONTROL_CHARACTER
        A required value contains a control character.
    FIELD_EMPTY
        A required value is empty after trimming whitespace.
    MANIFEST_READ
        The Cargo manifest cannot be read or parsed.
    MISSING_PACKAGE
        The manifest does not contain a package table.
    MISSING_AUTHORS
        The package does not contain a non-empty authors array.
    OUTPUT_UNSET
        GitHub Actions did not provide an output-file path.
    """

    FIELD_NOT_STRING = enum.auto()
    FIELD_CONTROL_CHARACTER = enum.auto()
    FIELD_EMPTY = enum.auto()
    MANIFEST_READ = enum.auto()
    MISSING_PACKAGE = enum.auto()
    MISSING_AUTHORS = enum.auto()
    OUTPUT_UNSET = enum.auto()


PACKAGE_METADATA_ERROR_MESSAGES = {
    PackageMetadataIssue.FIELD_NOT_STRING: "{detail} must be a string",
    PackageMetadataIssue.FIELD_CONTROL_CHARACTER: (
        "{detail} must not contain control characters"
    ),
    PackageMetadataIssue.FIELD_EMPTY: "{detail} must not be empty",
    PackageMetadataIssue.MISSING_PACKAGE: (
        "Cargo manifest must define a [package] table"
    ),
    PackageMetadataIssue.MISSING_AUTHORS: "package.authors must be a non-empty array",
    PackageMetadataIssue.OUTPUT_UNSET: "GITHUB_OUTPUT must name an output file",
}


class PackageMetadataError(ValueError):
    """Describe invalid or unreadable Cargo package metadata.

    Attributes
    ----------
    issue : PackageMetadataIssue
        The condition that caused metadata processing to fail.
    detail : object | None
        The field name or manifest read error associated with the issue.
    """

    def __init__(
        self, issue: PackageMetadataIssue, detail: object | None = None
    ) -> None:
        """Record the rejected metadata condition and optional field detail."""
        self.issue = issue
        self.detail = detail

    @typ.override
    def __str__(self) -> str:
        """Format a package metadata diagnostic for command-line output."""
        return _format_package_metadata_error(self.issue, self.detail)


class PackageMetadata(typ.TypedDict):
    """Hold the four package fields sourced from ``[package]``.

    Attributes
    ----------
    maintainer : str
        The first author from Cargo's ``authors`` array.
    homepage : str
        The project URL from Cargo's ``homepage`` field.
    license : str
        The licence identifier from Cargo's ``license`` field.
    description : str
        The package summary from Cargo's ``description`` field.
    """

    maintainer: str
    homepage: str
    license: str
    description: str


def _required_string(value: object, field: str) -> str:
    """Return one trimmed string field after rejecting unsafe values.

    Returns
    -------
    str
        The trimmed field value.

    Raises
    ------
    PackageMetadataError
        If the value is not a non-empty string without control characters.

    Examples
    --------
    >>> _required_string("  ISC  ", "package.license")
    'ISC'
    """
    match value:
        case str() as string_value:
            pass
        case _:
            raise PackageMetadataError(PackageMetadataIssue.FIELD_NOT_STRING, field)
    if any(unicodedata.category(character) == "Cc" for character in string_value):
        raise PackageMetadataError(PackageMetadataIssue.FIELD_CONTROL_CHARACTER, field)
    normalized = string_value.strip()
    if not normalized:
        raise PackageMetadataError(PackageMetadataIssue.FIELD_EMPTY, field)
    return normalized


def read_package_metadata(manifest_path: pathlib.Path) -> PackageMetadata:
    """Read the package's maintainer, homepage, licence, and description.

    Use the first author because Cargo permits multiple package authors while
    Debian and RPM package metadata each expose a single maintainer/packager.

    Parameters
    ----------
    manifest_path : pathlib.Path
        Cargo manifest whose ``[package]`` metadata supplies the package fields.

    Returns
    -------
    PackageMetadata
        The validated metadata fields from ``[package]``.

    Raises
    ------
    PackageMetadataError
        If the manifest cannot be read or any required package field is invalid.

    Examples
    --------
    >>> read_package_metadata(pathlib.Path("Cargo.toml"))["license"]
    'ISC'
    """
    try:
        document = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise PackageMetadataError(
            PackageMetadataIssue.MANIFEST_READ, (manifest_path, error)
        ) from error

    match document.get("package"):
        case dict() as package_value:
            package: dict[str, object] = package_value
        case _:
            raise PackageMetadataError(PackageMetadataIssue.MISSING_PACKAGE)

    match package.get("authors"):
        case [first_author, *_]:
            pass
        case _:
            raise PackageMetadataError(PackageMetadataIssue.MISSING_AUTHORS)

    return {
        "maintainer": _required_string(first_author, "package.authors[0]"),
        "homepage": _required_string(package.get("homepage"), "package.homepage"),
        "license": _required_string(package.get("license"), "package.license"),
        "description": _required_string(
            package.get("description"), "package.description"
        ),
    }


def _write_github_output(metadata: PackageMetadata) -> None:
    """Append the normalized metadata fields to GitHub Actions output.

    Raises
    ------
    PackageMetadataError
        If ``GITHUB_OUTPUT`` is unset or empty.

    Examples
    --------
    With ``GITHUB_OUTPUT=/tmp/outputs``, this appends one ``key=value`` line
    for each field in ``metadata``.
    """
    output_path = os.environ.get("GITHUB_OUTPUT")
    if not output_path:
        raise PackageMetadataError(PackageMetadataIssue.OUTPUT_UNSET)
    with pathlib.Path(output_path).open("a", encoding="utf-8", newline="\n") as output:
        output.writelines(f"{key}={value}\n" for key, value in metadata.items())


def main(argv: list[str] | None = None) -> int:
    """Write validated Cargo package metadata for a workflow step.

    Parameters
    ----------
    argv : list[str] | None
        Command-line arguments. ``None`` uses ``sys.argv[1:]``.

    Returns
    -------
    int
        ``0`` after writing the values or ``1`` after a metadata error.

    Examples
    --------
    ``main(["--manifest", "Cargo.toml"])`` reads the manifest and appends
    its four values to the path named by ``GITHUB_OUTPUT``.
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=pathlib.Path, required=True)
    arguments = parser.parse_args(argv)
    try:
        metadata = read_package_metadata(arguments.manifest)
        _write_github_output(metadata)
    except (OSError, PackageMetadataError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


def _format_package_metadata_error(
    issue: PackageMetadataIssue, detail: object | None
) -> str:
    """Format a static or detail-carrying package metadata failure.

    Returns
    -------
    str
        A concise diagnostic for the rejected package metadata.
    """
    if issue is PackageMetadataIssue.MANIFEST_READ:
        manifest_path, error = typ.cast("tuple[pathlib.Path, Exception]", detail)
        message = f"unable to read Cargo manifest {manifest_path}: {error}"
    else:
        message = PACKAGE_METADATA_ERROR_MESSAGES[issue].format(detail=detail)
    return message


if __name__ == "__main__":
    raise SystemExit(main())
