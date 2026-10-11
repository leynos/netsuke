"""Perform the two filesystem effects configuration initialization needs.

The gate creates both artefact files and both of their parents before it runs
any operation. These are the filesystem effects configuration initialization
performs, and they are isolated behind :class:`FileSystem` so the two halves of
the start-up can be observed apart: resolution can then be shown to perform no
filesystem access at all, and initialization to perform exactly these two
effects, in the shell's order, without a refusal having to be arranged on a
real filesystem.

The interface names whole *effects* rather than an ``open``/``mkdir`` pair. A
caller that opened a path itself would be choosing the mode and the truncation,
which is the behaviour under test; naming the effects keeps that choice here,
where the only thing expressible is what the shell's own ``mkdir -p`` and
``: >"$file"`` did.

The names are behaviour rather than mechanism because each differs from its
:mod:`pathlib` near-miss in exactly the way the contract cares about.
:meth:`FileSystem.make_parent` takes the ``mkdir`` operand, not the configured
file, because the shell asked coreutils ``dirname(1)`` for that operand; and
:meth:`FileSystem.truncate` takes the *configured spelling* rather than a
resolved :class:`pathlib.Path`, because ``Path("metrics.jsonl/")`` drops the
trailing slash that decides between a regular file and a directory.
"""

import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib


class FileSystem(typ.Protocol):
    """Define the two filesystem effects the gate's start-up performs.

    Both parameters are positional-only: an implementation is typically a plain
    function, and a callable that does not accept a keyword is only assignable
    to a member that does not require one.
    """

    def make_parent(self, directory: pathlib.Path, /) -> None:
        """Create *directory* and every missing parent, tolerating existence."""

    def truncate(self, configured: str, /) -> None:
        """Create *configured* empty, or truncate it, opening its own spelling."""


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


def production_filesystem() -> FileSystem:
    """Return the filesystem the gate uses unless a caller supplies another."""
    return OperatingSystemFileSystem()


type _Effect = cabc.Callable[[typ.Any], None]


class OperatingSystemFileSystem:
    """Perform both effects against the running system's own filesystem.

    The effects are delegated to the two module functions rather than
    reimplemented, so there is one spelling of each and the port's default can
    be read without the class standing between the reader and the behaviour.
    The attributes are assigned rather than shadowed by methods because a
    binding method can only be reached through an instance, and callers such as
    the fake here compare the two implementations directly.
    """

    make_parent: _Effect = staticmethod(make_parent)
    truncate: _Effect = staticmethod(truncate)
