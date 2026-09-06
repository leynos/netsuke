#!/usr/bin/env python3
"""Validate an untrusted LCOV coverage artefact before secret-bearing upload.

The pull-request workflow writes ``lcov.info`` and uploads it as an artefact.
The trusted submission workflow treats the downloaded directory as hostile
data: this command accepts exactly one regular UTF-8 LCOV file, constrains its
size, and parses only recognised LCOV records. It never executes, imports, or
resolves the coverage paths recorded in that data.
"""

import argparse
import enum
import pathlib
import re
import sys
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

EXPECTED_MEMBER = "lcov.info"
MAXIMUM_COVERAGE_BYTES = 16 * 1024 * 1024
LCOV_RECORDS = (
    re.compile(r"TN:[^\r\n]*"),
    re.compile(r"SF:[^\r\n]+"),
    re.compile(r"FN:[0-9]+,[^\r\n]+"),
    re.compile(r"FNDA:[0-9]+,[^\r\n]+"),
    re.compile(r"FNF:[0-9]+"),
    re.compile(r"FNH:[0-9]+"),
    re.compile(r"DA:[0-9]+,[0-9]+(?:,[^\r\n]+)?"),
    re.compile(r"LF:[0-9]+"),
    re.compile(r"LH:[0-9]+"),
    re.compile(r"BRDA:[0-9]+,[0-9]+,[0-9]+,(?:[0-9]+|-)"),
    re.compile(r"BRF:[0-9]+"),
    re.compile(r"BRH:[0-9]+"),
    re.compile(r"end_of_record"),
)
REQUIRED_LCOV_RECORDS = ("SF:", "DA:", "end_of_record")


class ValidationIssue(enum.Enum):
    """Identify one hostile artefact validation failure.

    Attributes
    ----------
    SYMLINK_DIRECTORY
        The supplied artefact directory is itself a symbolic link, so its
        resolved location is not controlled by the trusted workflow.
    NON_DIRECTORY
        The supplied artefact path is not a directory.
    UNEXPECTED_MEMBERS
        The artefact directory contains a member other than the expected
        single ``lcov.info`` file, or more members than the validator bounds.
    SYMLINK_MEMBER
        The coverage member is a symbolic link.
    NON_REGULAR_MEMBER
        The coverage member is not a regular file.
    ESCAPED_MEMBER
        The resolved coverage member leaves the artefact directory.
    EMPTY_REPORT
        The coverage report contains no records.
    INVALID_RECORD
        The coverage report contains a line that is not a recognised LCOV
        record.
    MISSING_RECORD
        The coverage report omits a required record type.
    MISSING_TERMINATOR
        The coverage report does not end with ``end_of_record``.
    OVERSIZED_REPORT
        The coverage report exceeds the configured size bound.
    NON_UTF8_REPORT
        The coverage report is not valid UTF-8 text.

    Examples
    --------
    >>> ValidationIssue.MISSING_RECORD.name
    'MISSING_RECORD'
    """

    SYMLINK_DIRECTORY = enum.auto()
    NON_DIRECTORY = enum.auto()
    UNEXPECTED_MEMBERS = enum.auto()
    SYMLINK_MEMBER = enum.auto()
    NON_REGULAR_MEMBER = enum.auto()
    ESCAPED_MEMBER = enum.auto()
    EMPTY_REPORT = enum.auto()
    INVALID_RECORD = enum.auto()
    MISSING_RECORD = enum.auto()
    MISSING_TERMINATOR = enum.auto()
    OVERSIZED_REPORT = enum.auto()
    NON_UTF8_REPORT = enum.auto()


class ValidationError(Exception):
    """Describe hostile artefact data that cannot reach CodeScene.

    Parameters
    ----------
    issue
        The validation issue that rejected the artefact.
    detail
        Optional display detail, such as the offending line number, member
        names, or an observed-versus-maximum size pair.

    Raises
    ------
    ValidationError
        Raised by the validator whenever hostile artefact data must not
        reach the secret-bearing submission step.

    Examples
    --------
    >>> str(ValidationError(ValidationIssue.EMPTY_REPORT))
    'coverage report is empty'
    """

    def __init__(self, issue: ValidationIssue, detail: object | None = None) -> None:
        """Record the validation issue and its optional display detail."""
        self.issue = issue
        self.detail = detail

    def __str__(self) -> str:
        """Format the validation issue for the command-line error contract."""
        return _format_validation_error(self.issue, self.detail)


STATIC_ERROR_MESSAGES = {
    ValidationIssue.SYMLINK_DIRECTORY: "artefact directory must not be a symlink",
    ValidationIssue.SYMLINK_MEMBER: (
        f"artefact member must not be a symlink: {EXPECTED_MEMBER}"
    ),
    ValidationIssue.NON_REGULAR_MEMBER: (
        f"artefact member must be a regular file: {EXPECTED_MEMBER}"
    ),
    ValidationIssue.ESCAPED_MEMBER: (
        f"artefact member escapes its directory: {EXPECTED_MEMBER}"
    ),
    ValidationIssue.EMPTY_REPORT: "coverage report is empty",
    ValidationIssue.MISSING_TERMINATOR: "coverage report must end with end_of_record",
    ValidationIssue.NON_UTF8_REPORT: "coverage report is not UTF-8 text",
}


def _format_validation_error(issue: ValidationIssue, detail: object | None) -> str:
    """Format a static or detail-carrying hostile-data failure."""
    match issue:
        case ValidationIssue.NON_DIRECTORY:
            return f"artefact directory is not a directory: {detail}"
        case ValidationIssue.UNEXPECTED_MEMBERS:
            return f"artefact must contain only {EXPECTED_MEMBER!r}, got {detail!r}"
        case ValidationIssue.INVALID_RECORD:
            return f"invalid LCOV record at line {detail}"
        case ValidationIssue.MISSING_RECORD:
            return f"coverage report is missing required {detail} record"
        case ValidationIssue.OVERSIZED_REPORT:
            return _oversized_report_message(detail)
        case _:
            return STATIC_ERROR_MESSAGES[issue]


def _oversized_report_message(detail: object | None) -> str:
    """Format the size-bound failure from the validator's controlled tuple."""
    match detail:
        case (int() as size, int() as maximum):
            return f"coverage report is {size} bytes; maximum is {maximum}"
        case _:
            return "coverage report exceeds its configured maximum"


def _is_within(path: pathlib.Path, directory: pathlib.Path) -> bool:
    """Return whether a resolved member remains inside its artefact directory."""
    try:
        path.relative_to(directory)
    except ValueError:
        return False
    return True


def _validated_directory(artifact_dir: pathlib.Path) -> pathlib.Path:
    """Resolve and validate the supplied non-link artefact directory."""
    if artifact_dir.is_symlink():
        raise ValidationError(ValidationIssue.SYMLINK_DIRECTORY)
    if not artifact_dir.is_dir():
        raise ValidationError(ValidationIssue.NON_DIRECTORY, artifact_dir)
    return artifact_dir.resolve(strict=True)


def _validated_coverage_path(directory: pathlib.Path) -> pathlib.Path:
    """Return the sole regular coverage member after validating its boundary."""
    members = _bounded_directory_members(directory)
    coverage_path = _sole_coverage_member(members)
    _reject_symlink_member(coverage_path)
    return _resolve_contained_regular_member(coverage_path, directory)


def _sole_coverage_member(members: list[pathlib.Path]) -> pathlib.Path:
    """Return the sole expected member or reject an unexpected directory shape."""
    names = sorted(member.name for member in members)
    if names != [EXPECTED_MEMBER]:
        raise ValidationError(ValidationIssue.UNEXPECTED_MEMBERS, names)
    return members[0]


def _bounded_directory_members(directory: pathlib.Path) -> list[pathlib.Path]:
    """Enumerate artefact members under an explicit member-count bound."""
    members: list[pathlib.Path] = []
    for member in directory.iterdir():
        members.append(member)
        if len(members) > 1:
            raise ValidationError(ValidationIssue.UNEXPECTED_MEMBERS, None)
    return members


def _reject_symlink_member(coverage_path: pathlib.Path) -> None:
    """Reject a symbolic-link coverage member before resolving it."""
    if coverage_path.is_symlink():
        raise ValidationError(ValidationIssue.SYMLINK_MEMBER)


def _resolve_contained_regular_member(
    coverage_path: pathlib.Path, directory: pathlib.Path
) -> pathlib.Path:
    """Resolve a regular member and require it to remain in its directory."""
    if not coverage_path.is_file():
        raise ValidationError(ValidationIssue.NON_REGULAR_MEMBER)
    resolved_path = coverage_path.resolve(strict=True)
    if not _is_within(resolved_path, directory):
        raise ValidationError(ValidationIssue.ESCAPED_MEMBER)
    return resolved_path


def _is_lcov_record(line: str) -> bool:
    """Return whether a line is one recognised LCOV record."""
    return any(record.fullmatch(line) is not None for record in LCOV_RECORDS)


def _first_invalid_lcov_line(lines: cabc.Sequence[str]) -> int | None:
    """Return the first one-based line number that is not an LCOV record."""
    return next(
        (
            line_number
            for line_number, line in enumerate(lines, start=1)
            if not _is_lcov_record(line)
        ),
        None,
    )


def _first_missing_lcov_record(lines: cabc.Sequence[str]) -> str | None:
    """Return the first required LCOV record that the report omits."""
    return next(
        (
            record
            for record in REQUIRED_LCOV_RECORDS
            if not any(line.startswith(record) for line in lines)
        ),
        None,
    )


_invalid_lcov_line_number = _first_invalid_lcov_line
_missing_lcov_record = _first_missing_lcov_record


def _raise_validation_error_if(
    *, should_raise: bool, issue: ValidationIssue, detail: object | None = None
) -> None:
    """Raise one validation error when its ordered condition is met."""
    if should_raise:
        raise ValidationError(issue, detail)


def _validate_lcov_text(text: str) -> None:
    """Reject empty, malformed, or incomplete LCOV text."""
    lines = text.splitlines()
    _raise_validation_error_if(
        should_raise=not lines,
        issue=ValidationIssue.EMPTY_REPORT,
    )
    invalid_line = _first_invalid_lcov_line(lines)
    _raise_validation_error_if(
        should_raise=invalid_line is not None,
        issue=ValidationIssue.INVALID_RECORD,
        detail=invalid_line,
    )
    missing_record = _first_missing_lcov_record(lines)
    _raise_validation_error_if(
        should_raise=missing_record is not None,
        issue=ValidationIssue.MISSING_RECORD,
        detail=missing_record,
    )
    _raise_validation_error_if(
        should_raise=lines[-1] != "end_of_record",
        issue=ValidationIssue.MISSING_TERMINATOR,
    )


def validate(artifact_dir: pathlib.Path) -> None:
    r"""Validate one downloaded LCOV artefact without executing its content.

    Parameters
    ----------
    artifact_dir
        The downloaded artefact directory to inspect. It must resolve to a
        real directory containing exactly one regular, non-symbolic
        ``lcov.info`` member inside its own boundary.

    Raises
    ------
    ValidationError
        When the directory shape, member type, member boundary, report size,
        report encoding, or LCOV record structure violates the hostile-data
        contract. ``OSError`` may also propagate when the artefact cannot be
        inspected on disk.

    Examples
    --------
    >>> import pathlib, tempfile
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     artefact = pathlib.Path(tmp)
    ...     (artefact / "lcov.info").write_text(
    ...         "TN:\\nSF:src/lib.rs\\nDA:1,1\\nend_of_record\\n",
    ...         encoding="utf-8",
    ...     )
    ...     validate(artefact)
    """
    directory = _validated_directory(artifact_dir)
    coverage_path = _validated_coverage_path(directory)
    size = coverage_path.stat().st_size
    if size > MAXIMUM_COVERAGE_BYTES:
        raise ValidationError(
            ValidationIssue.OVERSIZED_REPORT,
            (size, MAXIMUM_COVERAGE_BYTES),
        )
    try:
        text = coverage_path.read_text(encoding="utf-8")
    except UnicodeDecodeError as error:
        raise ValidationError(ValidationIssue.NON_UTF8_REPORT) from error
    _validate_lcov_text(text)


def main(argv: cabc.Sequence[str] | None = None) -> int:
    r"""Run the artefact gate and return its process exit status.

    Parameters
    ----------
    argv
        Command-line arguments to parse. ``None`` means the current process
        arguments, as :mod:`argparse` defines.

    Returns
    -------
    int
        ``0`` when the artefact passes validation, ``1`` when the artefact
        fails a hostile-data check, and ``2`` when the artefact cannot be
        inspected on disk.

    Examples
    --------
    >>> import pathlib, tempfile
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     artefact = pathlib.Path(tmp)
    ...     (artefact / "lcov.info").write_text(
    ...         "TN:\\nSF:src/lib.rs\\nDA:1,1\\nend_of_record\\n",
    ...         encoding="utf-8",
    ...     )
    ...     main(["--artifact-dir", str(artefact)])
    0
    """
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--artifact-dir",
        type=pathlib.Path,
        required=True,
        help="downloaded artefact directory to validate",
    )
    args = parser.parse_args(argv)
    try:
        validate(args.artifact_dir)
    except ValidationError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    except OSError as error:
        print(f"error: unable to inspect artefact: {error}", file=sys.stderr)
        return 2
    print("ok: validated hostile LCOV artefact")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
