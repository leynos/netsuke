"""Perform the filesystem effects the gate's start-up and verification need.

The gate creates both artefact files and both of their parents before it runs
any operation, and later reads the evidence manifest a producer published.
These are the filesystem effects it performs, and they are isolated behind
:class:`FileSystem` so the halves can be observed apart: resolution can then be
shown to perform no filesystem access at all, and initialization to perform
exactly its two effects, in the shell's order, without a refusal having to be
arranged on a real filesystem. The read exists for the same reason -- a
verifier that opened the manifest itself would be choosing the bound and the
decoding, which are the behaviour under test.

The interface names whole *effects* rather than an ``open``/``mkdir`` triple. A
caller that opened a path itself would be choosing the mode and the truncation,
which is the behaviour under test; naming the effects keeps that choice here,
where the only thing expressible is what the shell's own ``mkdir -p`` and
``: >"$file"`` did, plus a read bounded in bytes.

The names are behaviour rather than mechanism because each differs from its
:mod:`pathlib` near-miss in exactly the way the contract cares about.
:meth:`FileSystem.make_parent` takes the ``mkdir`` operand, not the configured
file, because the shell asked coreutils ``dirname(1)`` for that operand;
:meth:`FileSystem.truncate` and :meth:`FileSystem.read_bytes` take the
*configured spelling* rather than a resolved :class:`pathlib.Path`, because
``Path("metrics.jsonl/")`` drops the trailing slash that decides between a
regular file and a directory.
"""

import os
import stat
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

#: The flag that keeps opening a FIFO from waiting for its writer. Windows has
#: no FIFOs to wait for, so the flag's absence there is not a degradation.
_NON_BLOCKING: typ.Final[int] = getattr(os, "O_NONBLOCK", 0)


class EvidenceTooLargeError(OSError):
    """Raised when a file holds more bytes than the caller is willing to read."""

    def __init__(self, configured: str, limit: int) -> None:
        """Describe the document that exceeded the caller's byte bound."""
        super().__init__(f"{configured} is larger than the {limit}-byte evidence bound")


class NotRegularFileError(OSError):
    """Raised when a path the gate must read is not a regular file.

    A name can exist and still not be a document: a FIFO, a device, or a
    directory. Opening a FIFO without a writer waits, and a device can block
    inside the read, so neither is bounded by a byte count. Only a regular file
    is, and everything else is refused before a single byte is read.
    """

    def __init__(self, configured: str) -> None:
        """Describe the path that names something other than a regular file."""
        super().__init__(f"{configured} is not a regular file")


class FileSystem(typ.Protocol):
    """Define the filesystem effects the gate performs.

    Every parameter is positional-only: an implementation is typically a plain
    function, and a callable that does not accept a keyword is only assignable
    to a member that does not require one.
    """

    def make_parent(self, directory: pathlib.Path, /) -> None:
        """Create *directory* and every missing parent, tolerating existence."""

    def truncate(self, configured: str, /) -> None:
        """Create *configured* empty, or truncate it, opening its own spelling."""

    def read_bytes(self, configured: str, limit: int, /) -> bytes:
        """Read at most *limit* bytes from *configured*, opening its spelling."""


def make_parent(directory: pathlib.Path, /) -> None:
    """Create *directory* and every missing parent, tolerating existence."""
    directory.mkdir(parents=True, exist_ok=True)


def truncate(configured: str, /) -> None:
    r"""Create *configured* empty, opening the operator's own spelling.

    The configured string is the operand rather than a resolved
    :class:`pathlib.Path`. ``Path("metrics.jsonl/")`` normalizes the trailing
    slash away, so the resolved form would create the regular file that the
    shell's ``: >"metrics.jsonl/"`` refused with ``Is a directory``; opening the
    configured spelling refuses it too, and with the system's own sentence.
    """
    # ruff: ignore[builtin-open] - the configured string is the operand.
    with open(configured, "wb"):
        pass


def read_bytes(configured: str, limit: int, /) -> bytes:
    r"""Read at most *limit* bytes from *configured*, opening the operator's spelling.

    The bound is applied to what was read rather than to the size the
    filesystem reports, because that size can change between the question and
    the read. Reading one byte past the bound is what distinguishes a document
    of exactly *limit* bytes from a larger one without asking twice.

    The path is opened without blocking and the opened handle is inspected
    before any byte is read. A name that exists but is not a regular file --
    a FIFO with no writer, a device, a directory -- would otherwise wait inside
    the open or the read, where no byte bound applies and the gate could neither
    refuse nor finish. Inspecting the handle rather than the path is what makes
    the refusal about the object actually opened.

    An absent file propagates the :class:`FileNotFoundError` the open raised,
    which the caller tells apart from a file that exists but cannot be read.

    Parameters
    ----------
    configured
        The path exactly as the operator spelled it.
    limit
        The largest number of bytes the caller will accept.

    Returns
    -------
    bytes
        The file's contents, at most *limit* bytes long.

    Raises
    ------
    EvidenceTooLargeError
        If the file holds more than *limit* bytes.
    NotRegularFileError
        If *configured* names something other than a regular file.
    """
    descriptor = os.open(configured, os.O_RDONLY | _NON_BLOCKING)
    try:
        if not stat.S_ISREG(os.fstat(descriptor).st_mode):
            raise NotRegularFileError(configured)
        excerpt = _read_bounded(descriptor, limit)
    finally:
        os.close(descriptor)
    if len(excerpt) > limit:
        raise EvidenceTooLargeError(configured, limit)
    return excerpt


def _read_bounded(descriptor: int, limit: int) -> bytes:
    """Read at most *limit* + 1 bytes from an open regular file.

    A short read is not the end of the file: ``read`` may return fewer bytes
    than asked for, so the loop asks again until the bound is reached or the
    file ends.

    Returns
    -------
    bytes
        The bytes read, at most *limit* + 1 of them.
    """
    chunks: list[bytes] = []
    remaining = limit + 1
    while remaining > 0:
        chunk = os.read(descriptor, remaining)
        if not chunk:
            break
        chunks.append(chunk)
        remaining -= len(chunk)
    return b"".join(chunks)


def production_filesystem() -> FileSystem:
    """Return the filesystem the gate uses unless a caller supplies another."""
    return OperatingSystemFileSystem()


type _Effect = cabc.Callable[[typ.Any], None]
type _Read = cabc.Callable[[str, int], bytes]


class OperatingSystemFileSystem:
    """Perform every effect against the running system's own filesystem.

    The effects are delegated to the module functions rather than
    reimplemented, so there is one spelling of each and the port's default can
    be read without the class standing between the reader and the behaviour.
    The attributes are assigned rather than shadowed by methods because a
    binding method can only be reached through an instance, and callers such as
    the fakes here compare the two implementations directly.
    """

    make_parent: _Effect = staticmethod(make_parent)
    truncate: _Effect = staticmethod(truncate)
    read_bytes: _Read = staticmethod(read_bytes)
