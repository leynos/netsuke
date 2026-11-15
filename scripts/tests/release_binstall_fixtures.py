"""Build the downloaded-archive fixtures the binstall adapter tests share.

Every fixture is built in memory and written to a temporary directory, so no
test that uses these helpers reaches the network or depends on a checked-in
blob. One definition of a well-formed archive keeps the suite that reads,
extracts, and probes one telling the same story.

Run via ``make test-release-publication``.
"""

import io
import tarfile
import typing as typ

import release_consumer_archive_policy as policy
import release_digest_files

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

#: The payload every fixture archive carries as its root binary.
BINARY_BYTES = b"netsuke binary payload"

#: A sidecar name in the spelling cargo-binstall publishes.
SIDECAR_NAME = "netsuke-build-1.2.3-x86_64-unknown-linux-gnu.tar.gz.sha256"


def tarball(
    path: pathlib.Path,
    *,
    binary: bytes = BINARY_BYTES,
    extra: cabc.Sequence[tuple[str, bytes]] = (),
    kind: policy.MemberKind | None = None,
) -> pathlib.Path:
    """Write a gzip-compressed tar archive holding a binary and extras.

    Returns
    -------
    pathlib.Path
        The archive's path, once written.
    """
    with tarfile.open(path, "w:gz") as archive:
        entry = tarfile.TarInfo(policy.BINARY_NAME)
        entry.size = len(binary)
        if kind is policy.MemberKind.SYMLINK:
            entry.type = tarfile.SYMTYPE
            entry.linkname = "/etc/passwd"
            entry.size = 0
        _ = archive.addfile(entry, io.BytesIO(binary))
        for name, payload in extra:
            other = tarfile.TarInfo(name)
            other.size = len(payload)
            _ = archive.addfile(other, io.BytesIO(payload))
    return path


def digest_of(path: pathlib.Path) -> str:
    """Return the archive's digest as the trusted record would hold it.

    Returns
    -------
    str
        The lower-case hexadecimal SHA-256 of the file.
    """
    return release_digest_files.digest_of_file(path)
