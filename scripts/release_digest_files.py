"""Read artifact files and pass their digest facts to release policy.

This adapter owns blockwise file hashing and sidecar reads. It is used by the
package CLI, staged-digest collector, inventory verifier, and crates.io
download adapter; policy callers receive digests and sidecar text rather than
paths. Keeping this narrow avoids a second file-reading implementation for
large release archives.

Run via the release publication CLIs; behavioural coverage lives in
``scripts/tests/test_release_digest_files.py``.
"""

import hashlib
import typing as typ

import release_digest

if typ.TYPE_CHECKING:
    import pathlib


def digest_of_file(path: pathlib.Path) -> str:
    """Return the lowercase SHA-256 of a file, reading it in bounded blocks.

    Returns
    -------
    str
        The lowercase hexadecimal SHA-256 digest of the file.

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


def verify_file_against_sidecar(path: pathlib.Path) -> str:
    """Read and verify the sidecar beside ``path`` using pure digest policy.

    Returns
    -------
    str
        The lowercase digest shared by the file and its sidecar.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     asset = Path(tmp) / "asset.bin"
    ...     _ = asset.write_bytes(b"netsuke")
    ...     digest = digest_of_file(asset)
    ...     sidecar = Path(tmp) / "asset.bin.sha256"
    ...     _ = sidecar.write_text(
    ...         digest + "  dist/asset.bin" + chr(10), encoding="utf-8"
    ...     )
    ...     verify_file_against_sidecar(asset) == digest
    True
    """
    sidecar = path.with_name(f"{path.name}.sha256")
    actual = digest_of_file(path)
    text = sidecar.read_text(encoding="utf-8")
    return release_digest.verify_sidecar(path.name, actual, text)
