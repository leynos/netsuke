"""Compare published bytes with the digest a release recorded for them.

A release publishes a file and, beside it, a ``.sha256`` sidecar. The sidecar
is the release's own claim about the bytes; the file is the bytes. Neither
verifies alone: a sidecar never checked against the file it describes is only
a claim, and a file with no sidecar is a download a consumer cannot check.

The sidecars read here are written by ``scripts/package-artefact.sh`` with
``sha256sum``, or ``shasum -a 256`` where GNU coreutils is absent, so the
format is the one those tools emit: a 64-character lowercase hexadecimal
digest, a space, a single-character mode marker, then the path hashed. That
path is a machine-local path — ``artifacts/linux-x86_64/…`` in CI, a ``dist/``
prefix elsewhere — so it is deliberately **not** compared as a string. Only
the digest and the bare file name are portable.

Where a digest arrives from an untrusted source, :func:`constant_time_equal`
compares it without revealing, through timing, how much of the expected value
a guess matched.

Nothing here performs network I/O; callers supply already-downloaded paths.

Run via the release workflow's inventory verification; behavioural coverage
lives in ``scripts/tests/test_release_digest.py``.
"""

import hashlib
import hmac
import pathlib
import re

#: Hexadecimal digits in a SHA-256 digest.
DIGEST_HEX_LENGTH = 64

#: One checksum line: digest, a space, a mode marker, the hashed path.
#:
#: ``sha256sum`` writes two spaces for a text-mode read and `` *`` for a
#: binary-mode one, and ``shasum`` writes the same shape, so both markers are
#: accepted. The digest is matched case-insensitively and lowercased on the
#: way out, because producers differ on case while consumers compare hex.
_CHECKSUM_LINE = re.compile(
    rf"\A(?P<digest>[0-9a-fA-F]{{{DIGEST_HEX_LENGTH}}}) "
    r"(?P<marker>[ *])(?P<path>.+)\Z"
)


class DigestError(ValueError):
    """Report that a digest could not be read or did not match."""


class MalformedSidecarError(DigestError):
    """Report a sidecar that is not one well-formed, portable checksum line."""


class DigestMismatchError(DigestError):
    """Report bytes whose digest is not the one recorded for them."""


def digest_of_file(path: pathlib.Path) -> str:
    """Return the lowercase hexadecimal SHA-256 of the file at ``path``.

    Parameters
    ----------
    path
        File to hash.

    Returns
    -------
    str
        Sixty-four lowercase hexadecimal characters.

    Notes
    -----
    The file is read in blocks rather than whole. The inputs include release
    archives and a ``.crate``, so hashing one by loading it entirely would
    make the verifier's memory use a function of the artefact's size.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     sample = Path(tmp) / "hello.txt"
    ...     _ = sample.write_bytes(b"netsuke")
    ...     digest_of_file(sample)
    '79e4b991b14ef4d874ee27ff97dc10f5873a452496ac92c3d234e9ca46207b85'
    """
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(block)
    return hasher.hexdigest()


def parse_sidecar(text: str) -> tuple[str, str]:
    r"""Return the digest and bare file name a sidecar records.

    Parameters
    ----------
    text
        Contents of a ``.sha256`` file: one checksum line.

    Returns
    -------
    tuple[str, str]
        The lowercase hexadecimal digest, and the bare name of the file it
        was taken over.

    Raises
    ------
    MalformedSidecarError
        If ``text`` is not exactly one checksum line, if its digest is not 64
        hexadecimal digits, if it names no file, or if it is in the escaped
        form GNU tools emit for a path needing escape sequences.

    Notes
    -----
    A leading directory on the recorded path is discarded, not validated.
    ``sha256sum`` writes the path it was given, so a sidecar produced in CI
    records ``artifacts/linux-x86_64/netsuke-linux-x86_64`` while the same
    asset sits at the release root as ``netsuke-linux-x86_64``. Rejecting the
    leading directory would reject every sidecar the release produces.

    The escaped form — a leading backslash, used by GNU ``sha256sum`` when a
    path contains a newline or a backslash — is refused rather than decoded.
    It cannot legitimately appear: an asset name is required to be one bare
    component under both path flavours, so a name reaching here never holds a
    backslash or a newline. Refusing keeps a line that *does* carry one from
    being read as an ordinary path and parsed into a name the inventory
    would never accept.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     asset = Path(tmp) / "netsuke-linux-x86_64"
    ...     _ = asset.write_bytes(b"netsuke")
    ...     recorded = (
    ...         digest_of_file(asset)
    ...         + "  artifacts/linux-x86_64/netsuke-linux-x86_64"
    ...     )
    ...     digest, name = parse_sidecar(recorded)
    ...     (name, digest == digest_of_file(asset))
    ('netsuke-linux-x86_64', True)
    >>> parse_sidecar("not a checksum")  # doctest: +ELLIPSIS
    Traceback (most recent call last):
        ...
    release_digest.MalformedSidecarError: sidecar is not one SHA-256 checksum line: ...
    """
    line = text.strip()
    match = _CHECKSUM_LINE.match(line)
    if match is None:
        msg = f"sidecar is not one SHA-256 checksum line: {line!r}"
        raise MalformedSidecarError(msg)
    recorded = match.group("path")
    if recorded.startswith("\\"):
        msg = f"sidecar records an escaped path, which no asset name can be: {line!r}"
        raise MalformedSidecarError(msg)
    name = pathlib.PurePosixPath(recorded).name
    if not name:
        msg = f"sidecar names no file: {line!r}"
        raise MalformedSidecarError(msg)
    return match.group("digest").lower(), name


def constant_time_equal(left: str, right: str) -> bool:
    """Compare two digest strings without exiting early on a mismatch.

    Parameters
    ----------
    left
        One digest, usually the freshly computed one.
    right
        The other digest, usually the one a registry or a sidecar recorded.

    Returns
    -------
    bool
        Whether the two are equal, compared as bytes.

    Notes
    -----
    :func:`hmac.compare_digest` is used rather than ``==``. A published digest
    is not a secret, so this hides nothing here; it keeps the comparison
    independent of how many leading characters agree, which matters once the
    same helper is reused for values that are not public. Case is ignored,
    because ``sha256sum`` writes lowercase while other producers emit
    uppercase.

    Examples
    --------
    >>> constant_time_equal("AB" * 32, "ab" * 32)
    True
    >>> constant_time_equal("ab" * 32, "ab" * 31 + "cd")
    False
    """
    return hmac.compare_digest(left.lower().encode(), right.lower().encode())


def verify_file_against_sidecar(path: pathlib.Path) -> str:
    """Require ``path``'s bytes to match the sidecar written beside it.

    Parameters
    ----------
    path
        The published file, whose ``.sha256`` sibling is read.

    Returns
    -------
    str
        The verified digest, for a caller to compare against the trusted
        identity record's value.

    Raises
    ------
    MalformedSidecarError
        If the sidecar is malformed, or if it records a digest for a
        differently named file.
    DigestMismatchError
        If the file's digest is not the one its sidecar records.

    Notes
    -----
    The sidecar must describe *this* file. A sidecar copied or renamed beside
    the wrong file would otherwise confirm a digest that is perfectly valid
    for something else, so the recorded name is required to match before the
    bytes are trusted.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     asset = Path(tmp) / "asset.bin"
    ...     _ = asset.write_bytes(b"netsuke")
    ...     recorded = digest_of_file(asset) + "  dist/asset.bin" + chr(10)
    ...     _ = (Path(tmp) / "asset.bin.sha256").write_text(
    ...         recorded, encoding="utf-8"
    ...     )
    ...     verify_file_against_sidecar(asset) == digest_of_file(asset)
    True
    """
    sidecar = path.with_name(f"{path.name}.sha256")
    recorded_digest, recorded_name = parse_sidecar(sidecar.read_text(encoding="utf-8"))
    if recorded_name != path.name:
        msg = (
            f"{sidecar.name} records a digest for {recorded_name!r}, not for "
            f"{path.name!r}"
        )
        raise MalformedSidecarError(msg)
    actual_digest = digest_of_file(path)
    if not constant_time_equal(actual_digest, recorded_digest):
        msg = (
            f"{path.name} hashes to {actual_digest}, but its sidecar records "
            f"{recorded_digest}"
        )
        raise DigestMismatchError(msg)
    return actual_digest
