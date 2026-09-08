"""Inspect and materialize a hostile coverage ZIP archive without extraction.

The trusted coverage workflow calls this helper only after downloading a raw
GitHub Actions artifact. It inspects ZIP metadata before decompressing data and
writes exactly one validated LCOV member through a controlled output path.
"""

import dataclasses
import enum
import pathlib
import stat
import typing as typ
import zipfile

if typ.TYPE_CHECKING:
    import collections.abc as cabc


class ArchiveIssue(enum.StrEnum):
    """Identify one hostile ZIP archive validation failure."""

    NOT_ZIP = "not_zip"
    MEMBERS = "members"
    PATH = "path"
    TYPE = "type"
    SIZE = "size"
    ENCODING = "encoding"
    OUTPUT = "output"


ARCHIVE_ERROR_MESSAGES = {
    ArchiveIssue.NOT_ZIP: "coverage artefact is not a ZIP archive",
    ArchiveIssue.MEMBERS: "archive must contain exactly lcov.info",
    ArchiveIssue.PATH: "archive member path is unsafe",
    ArchiveIssue.TYPE: "archive member must be a regular non-link file",
    ArchiveIssue.SIZE: "archive uncompressed size exceeds limit",
    ArchiveIssue.ENCODING: "coverage report is not UTF-8 text",
    ArchiveIssue.OUTPUT: "validated output directory is unsafe",
}


class ArchiveValidationError(ValueError):
    """Describe a hostile ZIP archive that cannot be materialized safely."""

    def __init__(self, issue: ArchiveIssue) -> None:
        """Record the classified archive validation failure."""
        self.issue = issue
        super().__init__(ARCHIVE_ERROR_MESSAGES[issue])


@dataclasses.dataclass(frozen=True, slots=True)
class ArchiveContract:
    """Define the fixed member and validation constraints for one ZIP archive.

    Attributes
    ----------
    expected_member
        Sole permitted archive member name.
    maximum_uncompressed_bytes
        Maximum cumulative uncompressed archive member size.
    validate_text
        Pure callback that validates decoded member text before output.
    """

    expected_member: str
    maximum_uncompressed_bytes: int
    validate_text: cabc.Callable[[str], None]


def validate_and_materialize(
    archive_path: pathlib.Path,
    output_directory: pathlib.Path,
    contract: ArchiveContract,
) -> None:
    """Validate ZIP metadata and write its sole valid member to trusted output.

    Parameters
    ----------
    archive_path
        Regular non-link ZIP file selected by the outer artifact-directory
        validator.
    output_directory
        Empty trusted directory that receives only the validated member.
    contract
        Fixed member-name, size, and decoded-text validation contract.

    Raises
    ------
    ArchiveValidationError
        If ZIP metadata, member type, path, size, text encoding, or output
        shape violates the hostile-data contract.

    Notes
    -----
    ZIP metadata is checked before the member is opened. The implementation
    never invokes ZIP extraction helpers and writes validated bytes directly.
    """
    try:
        with zipfile.ZipFile(archive_path) as archive:
            member = _validated_member(archive.infolist(), contract)
            content = _bounded_member_content(archive, member, contract)
    except zipfile.BadZipFile as error:
        raise ArchiveValidationError(ArchiveIssue.NOT_ZIP) from error
    _validate_decoded_content(content, contract)
    _write_validated_member(output_directory, contract.expected_member, content)


def _validated_member(
    members: list[zipfile.ZipInfo], contract: ArchiveContract
) -> zipfile.ZipInfo:
    """Return the sole regular member after validating ZIP metadata."""
    names = [member.filename for member in members]
    if names != [contract.expected_member]:
        raise ArchiveValidationError(ArchiveIssue.MEMBERS)
    member = members[0]
    member_path = pathlib.PurePosixPath(member.filename)
    if member_path.is_absolute() or member_path.parts != (contract.expected_member,):
        raise ArchiveValidationError(ArchiveIssue.PATH)
    mode = member.external_attr >> 16
    if _is_non_regular_member(member, mode):
        raise ArchiveValidationError(ArchiveIssue.TYPE)
    if sum(info.file_size for info in members) > contract.maximum_uncompressed_bytes:
        raise ArchiveValidationError(ArchiveIssue.SIZE)
    return member


def _bounded_member_content(
    archive: zipfile.ZipFile,
    member: zipfile.ZipInfo,
    contract: ArchiveContract,
) -> bytes:
    """Read at most one byte beyond the validated uncompressed size limit."""
    with archive.open(member) as source:
        content = source.read(contract.maximum_uncompressed_bytes + 1)
    if len(content) > contract.maximum_uncompressed_bytes:
        raise ArchiveValidationError(ArchiveIssue.SIZE)
    return content


def _validate_decoded_content(content: bytes, contract: ArchiveContract) -> None:
    """Decode the bounded member and apply the caller's pure text validator."""
    try:
        text = content.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ArchiveValidationError(ArchiveIssue.ENCODING) from error
    contract.validate_text(text)


def _write_validated_member(
    output_directory: pathlib.Path, member_name: str, content: bytes
) -> None:
    """Write validated bytes only after preparing an empty non-link directory."""
    if _is_unsafe_output_directory(output_directory):
        raise ArchiveValidationError(ArchiveIssue.OUTPUT)
    output_directory.mkdir(parents=True, exist_ok=True)
    (output_directory / member_name).write_bytes(content)


def _is_non_regular_member(member: zipfile.ZipInfo, mode: int) -> bool:
    """Return whether a ZIP member is a directory, link, or non-regular file."""
    file_type = stat.S_IFMT(mode)
    return (
        member.is_dir()
        or stat.S_ISLNK(mode)
        or file_type
        not in {
            0,
            stat.S_IFREG,
        }
    )


def _is_unsafe_output_directory(output_directory: pathlib.Path) -> bool:
    """Return whether the trusted output destination is linked, nonempty, or invalid."""
    if output_directory.is_symlink():
        return True
    if not output_directory.exists():
        return False
    return not output_directory.is_dir() or any(output_directory.iterdir())
