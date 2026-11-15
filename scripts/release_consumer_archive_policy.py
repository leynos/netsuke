"""Judge the member layout of a cargo-binstall archive before extraction.

An archive downloaded from a public release URL is untrusted input. This module
decides, from member metadata alone, whether that archive is one a release
smoke may extract: every name must stay inside the extraction directory under
both path flavours, no member may be a link, device, or FIFO, and exactly one
regular, non-empty file may sit at the root as the shipped binary.

The decision is separated from the tar reader in
:mod:`release_binstall_archive` for two reasons. The rules are pure, so a unit
test can exercise every refusal without building a tarball. And the scan must
complete before the first byte is written: an archive whose last member is
unsafe must be refused while the extraction directory is still empty, or the
partial output becomes something a later step could observe.

Run through ``scripts/consumer_smoke_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_consumer_archive_policy.py``.
"""

import dataclasses
import enum
import typing as typ
from pathlib import PurePath, PurePosixPath, PureWindowsPath

from release_asset_names import walks_out_of_its_directory

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The only member the smoke extracts: the tool a public consumer runs.
BINARY_NAME = "netsuke"

#: Upper bound on the member count a release archive may declare.
#:
#: The archive ships one binary, a licence, shell completions, and manual
#: pages. A few hundred entries is generous; thousands describe a different
#: artefact, and reading their headers is unbounded work a hostile archive
#: could use to exhaust the runner before any refusal is reported.
MAX_MEMBERS = 4096

#: Upper bound on one member's declared size, and on their total.
#:
#: The scan does not read these bytes; the caps bound the copy that follows.
#: They are stated here so a metadata-only pass can refuse an archive that
#: would exceed them, rather than discovering it mid-write.
MAX_MEMBER_BYTES = 128 * 1024 * 1024
MAX_TOTAL_BYTES = 256 * 1024 * 1024

#: Both readings of a member name. ``C:\\netsuke.exe`` is one component to a
#: POSIX parser and a rooted path to a Windows one, and the extraction host is
#: whichever the runner provides.
_PATH_FLAVOURS = (PurePosixPath, PureWindowsPath)


class MemberKind(enum.StrEnum):
    """Classify one archive member by what extraction would create."""

    REGULAR = "regular"
    DIRECTORY = "directory"
    SYMLINK = "symlink"
    HARDLINK = "hardlink"
    DEVICE = "device"
    FIFO = "fifo"
    OTHER = "other"


class ArchiveLayoutError(ValueError):
    """Report an archive whose members cannot be extracted safely."""


class UnsafeMemberNameError(ArchiveLayoutError):
    """Report a member name that could escape the extraction directory."""


class DuplicateMemberError(ArchiveLayoutError):
    """Report two members that would occupy one path."""


class UnsafeMemberTypeError(ArchiveLayoutError):
    """Report a link, device, FIFO, or unknown member kind."""


class ArchiveTooLargeError(ArchiveLayoutError):
    """Report an archive whose declared members exceed the fixed caps."""


class MissingBinaryError(ArchiveLayoutError):
    """Report an archive without exactly one non-empty root binary."""


@dataclasses.dataclass(frozen=True, slots=True)
class ArchiveMember:
    """Describe one member without reading its bytes.

    Parameters
    ----------
    name
        The member name exactly as the archive header spelled it.
    kind
        What extraction would create.
    size
        The declared uncompressed size in bytes.
    """

    name: str
    kind: MemberKind
    size: int


@dataclasses.dataclass(frozen=True, slots=True)
class ArchiveLayout:
    """Record the validated layout the extractor may rely on.

    Parameters
    ----------
    binary_name
        The single root member to write, always :data:`BINARY_NAME`.
    binary_size
        That member's declared size, which the copy must reproduce exactly.
    member_count
        How many members were scanned.
    """

    binary_name: str
    binary_size: int
    member_count: int


def normalised_member_name(name: str) -> str:
    """Return the path a member would occupy, or refuse an unsafe name.

    Parameters
    ----------
    name
        The member name from the archive header.

    Returns
    -------
    str
        Its path relative to the extraction root, with empty and ``.``
        components removed.

    Raises
    ------
    UnsafeMemberNameError
        If the name is empty, contains a NUL or backslash, is absolute or
        drive-qualified under either path flavour, or contains ``..``.

    Notes
    -----
    Every ``..`` is refused, not only the one that escapes. ``a/../b``
    resolves inside the root, but it is a traversal component all the same,
    and accepting it means the duplicate check compares a spelling no honest
    producer writes.

    Examples
    --------
    >>> normalised_member_name("completions/netsuke.bash")
    'completions/netsuke.bash'
    >>> normalised_member_name("./netsuke")
    'netsuke'
    >>> normalised_member_name("../netsuke")  # doctest: +ELLIPSIS
    Traceback (most recent call last):
        ...
    release_consumer_archive_policy.UnsafeMemberNameError: archive member ...
    """
    if not name:
        msg = "archive member has an empty name"
        raise UnsafeMemberNameError(msg)
    if "\x00" in name:
        msg = f"archive member name {name!r} contains a NUL byte"
        raise UnsafeMemberNameError(msg)
    if "\\" in name:
        msg = f"archive member name {name!r} contains a backslash"
        raise UnsafeMemberNameError(msg)
    for flavour in _PATH_FLAVOURS:
        if _resolves_outside_root(flavour, name):
            msg = f"archive member name {name!r} resolves outside the extraction root"
            raise UnsafeMemberNameError(msg)
    parts = tuple(part for part in name.split("/") if part not in {"", "."})
    if not parts:
        msg = f"archive member name {name!r} names no path"
        raise UnsafeMemberNameError(msg)
    if ".." in parts:
        msg = f"archive member name {name!r} contains a traversal component"
        raise UnsafeMemberNameError(msg)
    return "/".join(parts)


def _resolves_outside_root(flavour: type[PurePath], name: str) -> bool:
    """Return whether one path flavour reads ``name`` as rooted or escaping.

    Returns
    -------
    bool
        Whether the flavour reads the name as absolute, driven, or as a path
        that walks out of its directory.

    Notes
    -----
    Both questions are asked of the same flavour because they are the same
    hazard seen twice: a drive letter roots a path on Windows the way a
    leading slash does on POSIX, and both leave the extraction directory.
    """
    parsed = flavour(name)
    if parsed.is_absolute() or parsed.root:
        return True
    return bool(parsed.drive) or walks_out_of_its_directory(parsed.parts)


def require_safe_layout(members: cabc.Sequence[ArchiveMember]) -> ArchiveLayout:
    """Return the validated layout, or refuse the archive.

    Parameters
    ----------
    members
        Every member the archive declares, in archive order.

    Returns
    -------
    ArchiveLayout
        The single root binary and the totals the scan established.

    Raises
    ------
    ArchiveTooLargeError
        If the archive declares more members than :data:`MAX_MEMBERS`.
    DuplicateMemberError
        If two members normalise to one path.

    Notes
    -----
    The per-member rules are applied by the helpers below, and report the same
    :class:`ArchiveLayoutError` family: a name that could escape the root, a
    link, device, or FIFO, an over-large member, or the absence of exactly one
    non-empty root :data:`BINARY_NAME`.

    Examples
    --------
    >>> layout = require_safe_layout([
    ...     ArchiveMember("netsuke", MemberKind.REGULAR, 7),
    ...     ArchiveMember("LICENSE", MemberKind.REGULAR, 3),
    ... ])
    >>> (layout.binary_name, layout.member_count)
    ('netsuke', 2)
    """
    if len(members) > MAX_MEMBERS:
        msg = (
            f"the archive declares {len(members)} members, more than the "
            f"{MAX_MEMBERS} an archive of this shape may carry"
        )
        raise ArchiveTooLargeError(msg)
    normalised: dict[str, ArchiveMember] = {}
    total = 0
    for member in members:
        name = normalised_member_name(member.name)
        _require_member_type(member, name)
        if name in normalised:
            msg = f"two archive members would occupy {name!r}"
            raise DuplicateMemberError(msg)
        normalised[name] = member
        total += _member_size(member, name)
    _require_total(total)
    return _require_binary(normalised)


def _require_member_type(member: ArchiveMember, name: str) -> None:
    """Refuse a member extraction would create as something other than a file.

    Raises
    ------
    UnsafeMemberTypeError
        If the member is not a regular file or a directory.
    """
    if member.kind in {MemberKind.REGULAR, MemberKind.DIRECTORY}:
        return
    msg = f"archive member {name!r} is a {member.kind.value}, which is never extracted"
    raise UnsafeMemberTypeError(msg)


def _member_size(member: ArchiveMember, name: str) -> int:
    """Return the member's declared size, after the cap checks below.

    Returns
    -------
    int
        The size the member header declares.

    Raises
    ------
    ArchiveTooLargeError
        If the member declares a negative size or exceeds the per-member cap.
    """
    if member.size < 0:
        msg = f"archive member {name!r} declares a negative size {member.size}"
        raise ArchiveTooLargeError(msg)
    if member.kind is MemberKind.REGULAR and member.size > MAX_MEMBER_BYTES:
        msg = (
            f"archive member {name!r} declares {member.size} bytes, more than "
            f"the {MAX_MEMBER_BYTES} one member may carry"
        )
        raise ArchiveTooLargeError(msg)
    return member.size


def _require_total(total: int) -> None:
    """Refuse an archive whose declared members exceed the total cap.

    Raises
    ------
    ArchiveTooLargeError
        If ``total`` exceeds :data:`MAX_TOTAL_BYTES`.
    """
    if total > MAX_TOTAL_BYTES:
        msg = (
            f"the archive declares {total} uncompressed bytes, more than the "
            f"{MAX_TOTAL_BYTES} the smoke will extract"
        )
        raise ArchiveTooLargeError(msg)


def _require_binary(members: dict[str, ArchiveMember]) -> ArchiveLayout:
    """Return the layout when exactly one root binary is present.

    Returns
    -------
    ArchiveLayout
        The single root binary, its declared size, and the member count.

    Raises
    ------
    MissingBinaryError
        If the root binary is absent, is not a regular file, or declares no
        bytes.
    """
    binary = members.get(BINARY_NAME)
    if binary is None:
        msg = f"the archive carries no root member named {BINARY_NAME!r}"
        raise MissingBinaryError(msg)
    if binary.kind is not MemberKind.REGULAR:
        msg = f"the root member {BINARY_NAME!r} is a {binary.kind.value}, not a file"
        raise MissingBinaryError(msg)
    if binary.size <= 0:
        msg = f"the root member {BINARY_NAME!r} is empty"
        raise MissingBinaryError(msg)
    return ArchiveLayout(
        binary_name=BINARY_NAME,
        binary_size=binary.size,
        member_count=len(members),
    )
