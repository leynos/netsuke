"""Test the member-layout rules applied before an archive is extracted.

The rules are decided from metadata alone, so every refusal here is exercised
without building a tarball. Reading real archives is covered in
``test_release_binstall_archive.py``.

Run via ``make test-release-publication``.
"""

import pytest
import release_consumer_archive_policy as policy
from release_consumer_archive_policy import (
    ArchiveMember,
    ArchiveTooLargeError,
    DuplicateMemberError,
    MemberKind,
    MissingBinaryError,
    UnsafeMemberNameError,
    UnsafeMemberTypeError,
)


def _regular(name: str, size: int = 1) -> ArchiveMember:
    """Build a regular-file member record."""
    return ArchiveMember(name=name, kind=MemberKind.REGULAR, size=size)


@pytest.mark.parametrize(
    ("name", "expected"),
    [
        ("netsuke", "netsuke"),
        ("./netsuke", "netsuke"),
        ("completions/netsuke.bash", "completions/netsuke.bash"),
        ("share//man/netsuke.1", "share/man/netsuke.1"),
    ],
)
def test_normalised_member_name_keeps_a_relative_path(name: str, expected: str) -> None:
    """Return the path a member occupies inside the extraction root."""
    assert policy.normalised_member_name(name) == expected, (
        f"{name!r} must normalise to its extraction-relative path"
    )


@pytest.mark.parametrize(
    "name",
    [
        "",
        "/netsuke",
        "\\netsuke",
        "C:\\netsuke.exe",
        "C:netsuke",
        "//server/share/netsuke",
        "..",
        "../netsuke",
        "a/../b",
        ".",
        "a\x00b",
        "completions\\netsuke.bash",
    ],
)
def test_normalised_member_name_refuses_a_name_that_could_escape(name: str) -> None:
    """Refuse every name that is absolute, rooted, or traverses upward."""
    with pytest.raises(UnsafeMemberNameError):
        policy.normalised_member_name(name)


def test_require_safe_layout_accepts_a_binary_beside_other_files() -> None:
    """Accept one root binary with licence and completion files around it."""
    layout = policy.require_safe_layout([
        _regular("netsuke", size=4096),
        _regular("LICENSE", size=11),
        _regular("completions/netsuke.bash", size=20),
        ArchiveMember("share", MemberKind.DIRECTORY, 0),
    ])

    assert (layout.binary_name, layout.binary_size, layout.member_count) == (
        "netsuke",
        4096,
        4,
    ), "the layout must record the single root binary and the member total"


@pytest.mark.parametrize(
    "kind",
    [
        MemberKind.SYMLINK,
        MemberKind.HARDLINK,
        MemberKind.DEVICE,
        MemberKind.FIFO,
        MemberKind.OTHER,
    ],
)
def test_require_safe_layout_refuses_a_member_extraction_would_create(
    kind: MemberKind,
) -> None:
    """Refuse a link, device, FIFO, or unknown member kind."""
    members = [
        _regular("netsuke"),
        ArchiveMember("payload", kind, 0),
    ]

    with pytest.raises(UnsafeMemberTypeError):
        policy.require_safe_layout(members)


def test_require_safe_layout_refuses_two_members_at_one_path() -> None:
    """Refuse two members that normalise to the same path."""
    with pytest.raises(DuplicateMemberError):
        policy.require_safe_layout([_regular("netsuke"), _regular("./netsuke")])


def test_require_safe_layout_refuses_too_many_members() -> None:
    """Refuse an archive declaring more members than the fixed cap."""
    members = [_regular("netsuke")] + [
        _regular(f"file-{index}") for index in range(policy.MAX_MEMBERS)
    ]

    with pytest.raises(ArchiveTooLargeError):
        policy.require_safe_layout(members)


def test_require_safe_layout_refuses_an_oversized_member() -> None:
    """Refuse a member whose declared size exceeds the per-member cap."""
    members = [_regular("netsuke"), _regular("huge", policy.MAX_MEMBER_BYTES + 1)]

    with pytest.raises(ArchiveTooLargeError):
        policy.require_safe_layout(members)


def test_require_safe_layout_refuses_a_negative_size() -> None:
    """Refuse a member declaring a negative size."""
    members = [_regular("netsuke"), _regular("odd", -1)]

    with pytest.raises(ArchiveTooLargeError):
        policy.require_safe_layout(members)


def test_require_safe_layout_refuses_a_total_over_the_cap() -> None:
    """Refuse an archive whose declared members exceed the total cap."""
    members = [
        _regular("netsuke"),
        _regular("first", policy.MAX_MEMBER_BYTES),
        _regular("second", policy.MAX_MEMBER_BYTES),
        _regular("third"),
    ]

    with pytest.raises(ArchiveTooLargeError):
        policy.require_safe_layout(members)


@pytest.mark.parametrize(
    "members",
    [
        [_regular("LICENSE")],
        [_regular("netsuke", size=0)],
        [ArchiveMember("netsuke", MemberKind.DIRECTORY, 0)],
        [],
    ],
    ids=["absent", "empty", "directory", "no-members"],
)
def test_require_safe_layout_refuses_an_archive_without_a_binary(
    members: list[ArchiveMember],
) -> None:
    """Refuse an archive with no single non-empty regular root binary."""
    with pytest.raises(MissingBinaryError):
        policy.require_safe_layout(members)
