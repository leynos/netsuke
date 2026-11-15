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

import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib


class EvidenceTooLargeError(OSError):
    """Raised when a file holds more bytes than the caller is willing to read."""

    def __init__(self, configured: str, limit: int) -> None:
        """Describe the document that exceeded the caller's byte bound."""
        super().__init__(f"{configured} is larger than the {limit}-byte evidence bound")


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

    An absent file propagates the :class:`FileNotFoundError` ``open`` raised,
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
    """
    # ruff: ignore[builtin-open] - the configured string is the operand.
    with open(configured, "rb") as handle:
        excerpt = handle.read(limit + 1)
    if len(excerpt) > limit:
        raise EvidenceTooLargeError(configured, limit)
    return excerpt


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
