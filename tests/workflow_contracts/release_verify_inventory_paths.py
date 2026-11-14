"""Compare the verifier's input paths by relationship, not by their text.

The verification exists because the trusted evidence and the downloaded bytes
come from different places, so a caller that names a download path as a trusted
input has already defeated it: the release would be compared with itself. The
action re-checks the relationship at run time, after canonicalizing every path;
the workflow contract checks the job's forwarded inputs before a run, so the
manifest cannot be read as safe when it is not.

The comparison is made on normalized path parts rather than on a string prefix,
because two spellings of one directory -- with a trailing slash, a redundant
separator, a `.` component, or a `..` component that steps back out of one --
are one directory, and a prefix test accepts some of those. This module owns
that comparison; the contract test supplies the inputs and the accepted default.

Run via ``make test-workflow-contracts``.
"""

import pathlib
import posixpath


def _normalised(value: object) -> pathlib.PurePath:
    """Return one input path with redundant separators and components removed.

    ``PurePosixPath`` alone keeps a ``..`` component, so ``trusted/../outside``
    would still list ``trusted`` among its parents and read as inside it.
    Collapsing each ``..`` against the component before it is what makes the
    comparison a statement about the directory a path names rather than about
    the order its parts happen to be written in.

    Returns
    -------
    pathlib.PurePath
        The path with its redundant separators, `.` components and parent
        components collapsed.
    """
    return pathlib.PurePosixPath(posixpath.normpath(str(value)))


def _inside(parent: pathlib.PurePath, child: pathlib.PurePath) -> bool:
    """Return whether ``child`` is strictly below ``parent``."""
    return parent != child and parent in child.parents


def _overlap(first: pathlib.PurePath, second: pathlib.PurePath) -> bool:
    """Return whether the two paths name one directory or nest either way."""
    return first == second or _inside(first, second) or _inside(second, first)


def assert_disjoint_inputs(inputs: dict[str, object]) -> None:
    """Require trusted inputs inside their root and disjoint from the rest.

    Parameters
    ----------
    inputs
        The values the job forwards to the verification action. The
        ``trusted-directory``, ``declaration-file``, ``trusted-record-file``,
        ``download-directory`` and ``output-path`` entries are compared.

    Raises
    ------
    AssertionError
        If a trusted file is not strictly inside the trusted directory, if any
        pair of the trusted directory, the download directory and the output
        path is equal or nested either way, or if the output would be written
        into the download directory.
    """
    trusted = _normalised(inputs["trusted-directory"])
    download = _normalised(inputs["download-directory"])
    output = _normalised(inputs["output-path"])
    declaration = _normalised(inputs["declaration-file"])
    record = _normalised(inputs["trusted-record-file"])
    problem = (
        _inside_message("declaration-file", declaration, trusted)
        or _inside_message("trusted-record-file", record, trusted)
        or _disjoint_message("download-directory", download, trusted)
        or _disjoint_message("output-path", output, trusted)
        or _overlap_message("output-path", output, "download-directory", download)
    )
    if problem is not None:
        raise AssertionError(problem)


def _inside_message(
    key: str, value: pathlib.PurePath, trusted: pathlib.PurePath
) -> str | None:
    """Return the refusal for a trusted file that names something outside."""
    if _inside(trusted, value):
        return None
    return f"{key} ({value}) must be inside trusted-directory ({trusted})"


def _disjoint_message(
    label: str, value: pathlib.PurePath, trusted: pathlib.PurePath
) -> str | None:
    """Return the refusal for a path that meets the trusted directory."""
    if not _overlap(trusted, value):
        return None
    return f"{label} ({value}) must be disjoint from trusted-directory ({trusted})"


def _overlap_message(
    label: str,
    value: pathlib.PurePath,
    other_label: str,
    other: pathlib.PurePath,
) -> str | None:
    """Return the refusal for two labelled paths that name related directories."""
    if not _overlap(value, other):
        return None
    return f"{label} ({value}) must not overlap {other_label} ({other})"
