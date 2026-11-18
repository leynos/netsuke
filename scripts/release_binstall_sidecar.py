"""Verify the published checksum sidecar of a downloaded archive.

A cargo-binstall release publishes each archive beside a ``.sha256`` file, and
that sidecar is what a consumer's installer reads. An archive whose sidecar
describes different bytes is a release a ``cargo-binstall`` user cannot
install even though the archive itself is sound, so the smoke checks the
sidecar as well as the archive's trusted digest.

The sidecar's *own* digest is checked by the caller against the trusted record
before this module reads its text, so a substituted sidecar is refused before
its contents are compared with anything. Everything here is about that one
line of text: reading it under a size cap, decoding it, and comparing it with
the archive's digest.

Run through ``scripts/consumer_smoke_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_binstall_sidecar.py``.
"""

import typing as typ

import release_digest
import release_digest_files
from release_binstall_archive import ArchiveRefusedError

if typ.TYPE_CHECKING:
    import pathlib

#: Largest checksum sidecar that will be read. One checksum line is under a
#: hundred bytes; the bound stops a hostile body being read into memory.
MAX_SIDECAR_BYTES = 4096


class SidecarVerificationError(ArchiveRefusedError, release_digest.DigestError):
    """Report a sidecar that does not describe the downloaded archive."""


def verify_sidecar(archive_path: pathlib.Path, sidecar_path: pathlib.Path) -> str:
    """Require the published sidecar to describe the downloaded archive.

    Returns
    -------
    str
        The digest the sidecar records, which equals the archive's own.

    Raises
    ------
    SidecarVerificationError
        If the sidecar is empty, unreadable, not UTF-8, malformed, names a
        different asset, or records a different digest.

    Notes
    -----
    The sidecar's *own* digest is checked by the caller against the trusted
    record before this runs, so a substituted sidecar is refused before its
    text is compared with anything.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> import release_digest_files
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     archive = Path(tmp) / "netsuke.tar.gz"
    ...     _ = archive.write_bytes(b"netsuke")
    ...     digest = release_digest_files.digest_of_file(archive)
    ...     sidecar = Path(tmp) / "netsuke.tar.gz.sha256"
    ...     _ = sidecar.write_text(
    ...         digest + "  netsuke.tar.gz" + chr(10), encoding="utf-8"
    ...     )
    ...     verify_sidecar(archive, sidecar) == digest
    True
    """
    text = _read_sidecar_text(sidecar_path)
    actual = release_digest_files.digest_of_file(archive_path)
    try:
        return release_digest.verify_sidecar(archive_path.name, actual, text)
    except release_digest.DigestError as error:
        msg = f"the sidecar does not verify {archive_path.name}: {error}"
        raise SidecarVerificationError(msg) from error


def _read_sidecar_text(sidecar_path: pathlib.Path) -> str:
    """Return the sidecar's text, or refuse one that is not one checksum line.

    Returns
    -------
    str
        The decoded sidecar body.

    Notes
    -----
    Every refusal is :class:`SidecarVerificationError`, raised by the two
    helpers this composes.
    """
    return _decode_sidecar(sidecar_path, _read_sidecar_bytes(sidecar_path))


def _read_sidecar_bytes(sidecar_path: pathlib.Path) -> bytes:
    """Return the sidecar's body, refusing one that is unreadable or not one line.

    Returns
    -------
    bytes
        The body, which is neither empty nor larger than the cap.

    Raises
    ------
    SidecarVerificationError
        If the sidecar cannot be read, is empty, or is larger than
        :data:`MAX_SIDECAR_BYTES`.

    Notes
    -----
    The body is read one byte past the cap so an oversized file is recognised
    from its first read rather than after it has been read whole.
    """
    try:
        with sidecar_path.open("rb") as handle:
            text_bytes = handle.read(MAX_SIDECAR_BYTES + 1)
    except OSError as error:
        msg = f"the sidecar {sidecar_path.name} could not be read: {error}"
        raise SidecarVerificationError(msg) from error
    if not text_bytes:
        msg = f"the sidecar {sidecar_path.name} is empty"
        raise SidecarVerificationError(msg)
    if len(text_bytes) > MAX_SIDECAR_BYTES:
        msg = (
            f"the sidecar {sidecar_path.name} is larger than "
            f"{MAX_SIDECAR_BYTES} bytes, so it is not one checksum line"
        )
        raise SidecarVerificationError(msg)
    return text_bytes


def _decode_sidecar(sidecar_path: pathlib.Path, text_bytes: bytes) -> str:
    """Decode the sidecar's body, refusing bytes that are not UTF-8 text."""
    try:
        return text_bytes.decode("utf-8")
    except UnicodeDecodeError as error:
        msg = f"the sidecar {sidecar_path.name} is not UTF-8 text: {error}"
        raise SidecarVerificationError(msg) from error
