"""Map tar headers into the archive policy's member records.

The policy in :mod:`release_consumer_archive_policy` decides whether an
archive's layout may be extracted, and it decides from its own records rather
than from ``tarfile`` types. This module is the translation between the two:
one header becomes one :class:`release_consumer_archive_policy.ArchiveMember`
carrying the name the header spelled, the kind extraction would create, and the
declared size.

Keeping the mapping here leaves :mod:`release_binstall_archive` to read and
verify the download, and keeps the classification table beside the policy it
feeds. Nothing here touches the archive's bytes; a header is metadata only, and
the copy that follows is owned by :mod:`release_binstall_extract`.

Run through ``scripts/consumer_smoke_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_binstall_archive.py``.
"""

import typing as typ

import release_consumer_archive_policy as policy

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import tarfile


def require_layout(
    headers: cabc.Sequence[tarfile.TarInfo],
) -> policy.ArchiveLayout:
    """Return the policy's layout for headers an archive declared.

    Returns
    -------
    release_consumer_archive_policy.ArchiveLayout
        The single root binary and the member totals the policy accepted.

    Notes
    -----
    The policy owns every safety decision, so an unsafe layout's
    :class:`release_consumer_archive_policy.ArchiveLayoutError` propagates from
    :func:`release_consumer_archive_policy.require_safe_layout` unchanged.
    """
    return policy.require_safe_layout([_member(entry) for entry in headers])


def _member(entry: tarfile.TarInfo) -> policy.ArchiveMember:
    """Map one tar header into the policy's member record.

    Returns
    -------
    release_consumer_archive_policy.ArchiveMember
        The member name, its kind, and its declared size.

    Examples
    --------
    >>> import tarfile
    >>> entry = tarfile.TarInfo("netsuke")
    >>> _member(entry).kind
    <MemberKind.REGULAR: 'regular'>
    """
    return policy.ArchiveMember(
        name=entry.name, kind=_member_kind(entry), size=entry.size
    )


def _member_kind(entry: tarfile.TarInfo) -> policy.MemberKind:
    """Classify one tar header without reading its data.

    Returns
    -------
    release_consumer_archive_policy.MemberKind
        The kind extraction would create for this member.

    Notes
    -----
    A table rather than a chain of branches: the header predicates are
    mutually exclusive, so the first match is the answer, and a kind added to
    the policy fails visibly here instead of silently classifying as
    ``other``.
    """
    recognised = (
        (entry.isreg, policy.MemberKind.REGULAR),
        (entry.isdir, policy.MemberKind.DIRECTORY),
        (entry.issym, policy.MemberKind.SYMLINK),
        (entry.islnk, policy.MemberKind.HARDLINK),
        (entry.isdev, policy.MemberKind.DEVICE),
        (entry.isfifo, policy.MemberKind.FIFO),
    )
    return next(
        (kind for predicate, kind in recognised if predicate()),
        policy.MemberKind.OTHER,
    )


__all__ = ["require_layout"]
