#!/usr/bin/env python3
"""Validate package metadata against Cargo.

Run from the repository root after a package build:
$ python3 scripts/validate_linux_package_metadata.py --dist dist \
  --manifest Cargo.toml --package-name netsuke --license-file LICENSE
"""

import argparse
import collections.abc as cabc
import dataclasses
import enum
import hashlib
import io
import pathlib
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - package queries.
import sys
import tarfile
import typing as typ

from cargo_package_metadata import PackageMetadataError, read_package_metadata

type SubprocessRunner = cabc.Callable[..., subprocess.CompletedProcess[bytes]]


class PackageValidationIssue(enum.Enum):
    """Name the package-validation failure cases reported by this script."""

    MISSING_TOOL = "tool {!r} was not found on PATH"
    PACKAGE_COUNT = "{}: expected exactly one direct {} package, found {}"
    PACKAGE_FILE = "{}: expected a regular, non-symlink package file"
    DIST_DIRECTORY = "{}: expected a regular, non-symlink distribution directory"
    COMMAND_LAUNCH = "{}: field {}: unable to run inspection command: {}"
    COMMAND_FAILED = "{}: field {}: inspection command failed with status {}: {}"
    INVALID_UTF8 = "{}: field {}: inspection output is not UTF-8"
    PACKAGE_NAME = "invalid package name for Debian copyright lookup: {!r}"
    COPYRIGHT_COUNT = "{}: field Debian copyright: expected {}; actual {}"
    COPYRIGHT_FILE = (
        "{}: field Debian copyright: expected a regular file matching {}; "
        "actual non-regular archive member"
    )
    COPYRIGHT_READ = "{}: field Debian copyright: expected {}; actual unreadable file"
    INVALID_ARCHIVE = "{}: field Debian copyright: invalid package file list: {}"


class PackageValidationError(ValueError):
    """Carry a classified package-validation issue and its display values."""

    def __init__(
        self, issue: PackageValidationIssue, context: tuple[object, ...]
    ) -> None:
        """Store the issue and values used to render its diagnostic."""
        self.issue = issue
        self.context = context
        super().__init__(str(self))

    @typ.override
    def __str__(self) -> str:
        """Render the issue with its associated diagnostic values."""
        return self.issue.value.format(*self.context)


@dataclasses.dataclass(frozen=True, slots=True)
class _PackageValidationRequest:
    """Carry CLI paths from ``main`` into this script's package validator."""

    dist: pathlib.Path
    manifest: pathlib.Path
    package_name: str
    licence_file: pathlib.Path


@dataclasses.dataclass(frozen=True, slots=True)
class _PackageInspector:
    """Bind package queries to this script's command and injectable runner.

    Keep this boundary local to the validator; Debian and RPM readers share it,
    and tests supply its runner through ``main``.
    """

    command: str
    package: pathlib.Path
    runner: SubprocessRunner

    def run(self, argv: list[str], field: str) -> bytes:
        """Run one query and return stdout bytes or raise a classified failure."""
        try:
            result = self.runner(argv, capture_output=True, check=False)
        except OSError as error:
            raise PackageValidationError(
                PackageValidationIssue.COMMAND_LAUNCH,
                (self.package.name, field, error),
            ) from error
        if result.returncode:
            detail = result.stderr.decode("utf-8", errors="replace").strip()
            raise PackageValidationError(
                PackageValidationIssue.COMMAND_FAILED,
                (self.package.name, field, result.returncode, detail),
            )
        return result.stdout

    def text(self, argv: list[str], field: str) -> str:
        """Decode one query's UTF-8 stdout, such as ``b"ISC"`` as ``"ISC"``."""
        output = self.run(argv, field)
        try:
            return output.decode("utf-8").strip()
        except UnicodeDecodeError as error:
            raise PackageValidationError(
                PackageValidationIssue.INVALID_UTF8, (self.package.name, field)
            ) from error


def _required_tool(name: str) -> str:
    """Return a tool path such as ``/usr/bin/rpm`` or name the missing tool."""
    command = shutil.which(name)
    if command is None:
        raise PackageValidationError(PackageValidationIssue.MISSING_TOOL, (name,))
    return command


def _find_package(dist: pathlib.Path, suffix: str) -> pathlib.Path:
    """Find the sole ``.deb`` or ``.rpm`` in ``dist``; reject duplicates and links."""
    candidates = sorted(path for path in dist.iterdir() if path.suffix == suffix)
    if len(candidates) != 1:
        raise PackageValidationError(
            PackageValidationIssue.PACKAGE_COUNT,
            (dist, suffix, len(candidates)),
        )
    package = candidates[0]
    if package.is_symlink() or not package.is_file():
        raise PackageValidationError(PackageValidationIssue.PACKAGE_FILE, (package,))
    return package


def _package_files(dist: pathlib.Path) -> tuple[pathlib.Path, pathlib.Path]:
    """Return the unique ``.deb`` and ``.rpm`` files from ``dist`` in that order."""
    if dist.is_symlink() or not dist.is_dir():
        raise PackageValidationError(PackageValidationIssue.DIST_DIRECTORY, (dist,))
    return _find_package(dist, ".deb"), _find_package(dist, ".rpm")


def _read_debian_field(inspector: _PackageInspector, field: str) -> str:
    """Read one Debian control value such as ``Maintainer`` using ``dpkg-deb``."""
    return inspector.text(
        [inspector.command, "--field", str(inspector.package), field], field
    )


def _read_rpm_tag(inspector: _PackageInspector, tag: str) -> str:
    """Read an RPM tag and map RPM's ``(none)`` marker to an empty value."""
    value = inspector.text(
        [
            inspector.command,
            "-qp",
            "--queryformat",
            f"%{{{tag}}}",
            str(inspector.package),
        ],
        tag,
    )
    return "" if value == "(none)" else value


def _copyright_fingerprint(contents: bytes) -> str:
    """Show bytes as SHA-256 plus size, e.g. ``b"ISC"`` as a 3-byte value."""
    return f"sha256:{hashlib.sha256(contents).hexdigest()} ({len(contents)} bytes)"


def _copyright_member_path(package_name: str) -> str:
    """Map ``netsuke`` to its Debian copyright member path."""
    invalid_name_issue = PackageValidationIssue.PACKAGE_NAME
    if not package_name or package_name in {".", ".."}:
        raise PackageValidationError(invalid_name_issue, (package_name,))
    if any(character in package_name for character in ("/", "\\")):
        raise PackageValidationError(invalid_name_issue, (package_name,))
    return f"usr/share/doc/{package_name}/copyright"


def _read_debian_copyright(
    inspector: _PackageInspector, package_name: str, expected: bytes
) -> bytes | None:
    """Read Debian's copyright member safely, returning ``None`` when missing."""
    member_path = _copyright_member_path(package_name)
    archive_contents = inspector.run(
        [inspector.command, "--fsys-tarfile", str(inspector.package)], "copyright"
    )
    try:
        with tarfile.open(fileobj=io.BytesIO(archive_contents), mode="r:*") as archive:
            matching = [
                member
                for member in archive.getmembers()
                if member.name.removeprefix("./") == member_path
            ]
            if not matching:
                return None
            if len(matching) != 1:
                raise PackageValidationError(
                    PackageValidationIssue.COPYRIGHT_COUNT,
                    (
                        inspector.package.name,
                        _copyright_fingerprint(expected),
                        f"{len(matching)} members",
                    ),
                )
            member = matching[0]
            if not member.isfile():
                raise PackageValidationError(
                    PackageValidationIssue.COPYRIGHT_FILE,
                    (inspector.package.name, _copyright_fingerprint(expected)),
                )
            contents = archive.extractfile(member)
            if contents is None:
                raise PackageValidationError(
                    PackageValidationIssue.COPYRIGHT_READ,
                    (inspector.package.name, _copyright_fingerprint(expected)),
                )
            with contents:
                return contents.read()
    except tarfile.TarError as error:
        raise PackageValidationError(
            PackageValidationIssue.INVALID_ARCHIVE, (inspector.package.name, error)
        ) from error


def _record_comparison(
    errors: list[str], field_label: str, expected: str, actual: str
) -> None:
    """Report an empty or unequal field with both expected and actual values."""
    if not actual:
        outcome = "is empty"
    elif actual != expected:
        outcome = "does not match"
    else:
        return
    errors.append(f"{field_label} {outcome}: expected {expected!r}; actual {actual!r}")


def _record_copyright_comparison(
    errors: list[str], package_name: str, expected: bytes, actual: bytes | None
) -> None:
    """Report missing, empty or differing Debian licence bytes by fingerprint."""
    expected_value = _copyright_fingerprint(expected)
    if actual is None:
        outcome, actual_value = "is missing", "missing"
    elif not actual:
        outcome, actual_value = "is empty", _copyright_fingerprint(actual)
    elif actual != expected:
        outcome, actual_value = "does not match", _copyright_fingerprint(actual)
    else:
        return
    errors.append(
        f"{package_name}: field Debian copyright {outcome}: "
        f"expected {expected_value}; actual {actual_value}"
    )


def _summary_line(description: str) -> str:
    r"""Use the first line of a multiline package description as its summary."""
    return description.splitlines()[0] if description else ""


def _validate_debian(
    inspector: _PackageInspector,
    package_name: str,
    metadata: cabc.Mapping[str, str],
    licence_contents: bytes,
) -> list[str]:
    """Compare Debian ``Maintainer``, ``Homepage``, ``Description`` and copyright."""
    expected_fields = {
        "Maintainer": metadata["maintainer"],
        "Homepage": metadata["homepage"],
        "Description": _summary_line(metadata["description"]),
    }
    errors: list[str] = []
    for field, expected in expected_fields.items():
        try:
            actual = _read_debian_field(inspector, field)
        except PackageValidationError as error:
            errors.append(str(error))
        else:
            _record_comparison(
                errors, f"{inspector.package.name}: field {field}", expected, actual
            )
    try:
        actual_copyright = _read_debian_copyright(
            inspector, package_name, licence_contents
        )
    except PackageValidationError as error:
        errors.append(str(error))
    else:
        _record_copyright_comparison(
            errors, inspector.package.name, licence_contents, actual_copyright
        )
    return errors


def _validate_rpm(
    inspector: _PackageInspector, metadata: cabc.Mapping[str, str]
) -> list[str]:
    """Compare RPM tags, e.g. ``_read_rpm_tag(inspector, "LICENSE")`` reads ISC."""
    expected_fields = {
        "PACKAGER": metadata["maintainer"],
        "URL": metadata["homepage"],
        "LICENSE": metadata["license"],
        "SUMMARY": _summary_line(metadata["description"]),
        "DESCRIPTION": metadata["description"],
    }
    errors: list[str] = []
    for tag, expected in expected_fields.items():
        try:
            actual = _read_rpm_tag(inspector, tag)
        except PackageValidationError as error:
            errors.append(str(error))
        else:
            _record_comparison(
                errors, f"{inspector.package.name}: field {tag}", expected, actual
            )
    return errors


def validate_linux_package_metadata(
    request: _PackageValidationRequest,
    *,
    runner: SubprocessRunner | None = None,
) -> list[str]:
    """Validate one built Debian and RPM package against Cargo metadata.

    Returns
    -------
    list[str]
        Empty when package fields match; otherwise one diagnostic per mismatch.

    Examples
    --------
    Matching package headers and Debian copyright return an empty list.
    """
    metadata = read_package_metadata(request.manifest)
    debian_package, rpm_package = _package_files(request.dist)
    expected_licence = request.licence_file.read_bytes()
    selected_runner = runner if runner is not None else subprocess.run
    debian_inspector = _PackageInspector(
        _required_tool("dpkg-deb"), debian_package, selected_runner
    )
    rpm_inspector = _PackageInspector(
        _required_tool("rpm"), rpm_package, selected_runner
    )
    errors = _validate_debian(
        debian_inspector,
        request.package_name,
        metadata,
        expected_licence,
    )
    errors.extend(_validate_rpm(rpm_inspector, metadata))
    return errors


def main(
    argv: list[str] | None = None,
    *,
    runner: SubprocessRunner | None = None,
) -> int:
    """Run package metadata validation and return its command status.

    Returns
    -------
    int
        ``0`` when both packages match or ``1`` on validation failure.

    Examples
    --------
    ``main(["--dist", "dist", ...])`` returns ``0`` for matching packages.
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dist", type=pathlib.Path, required=True)
    parser.add_argument("--manifest", type=pathlib.Path, required=True)
    parser.add_argument("--package-name", required=True)
    parser.add_argument("--license-file", type=pathlib.Path, required=True)
    arguments = parser.parse_args(argv)
    request = _PackageValidationRequest(
        arguments.dist,
        arguments.manifest,
        arguments.package_name,
        arguments.license_file,
    )
    try:
        errors = validate_linux_package_metadata(request, runner=runner)
    except (OSError, PackageMetadataError, PackageValidationError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    print("ok: Debian and RPM package metadata match Cargo.toml")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
