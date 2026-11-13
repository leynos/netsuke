"""Compare the verifier's input paths by relationship, not by their text.

The verification exists because the trusted evidence and the downloaded bytes
come from different places, so a caller that names a download path as a trusted
input has already defeated it: the release would be compared with itself. The
action re-checks the relationship at run time, after canonicalizing every path;
the workflow contract checks the job's forwarded inputs before a run, so the
manifest cannot be read as safe when it is not.

The comparison is made on normalized path parts rather than on a string prefix,
because two spellings of one directory -- with a trailing slash, a redundant
separator, or a `.` component -- are one directory, and a prefix test accepts
some of those. This module owns that comparison; the contract test supplies the
inputs and the accepted default.

Run via ``make test-workflow-contracts``.
"""

import pathlib


def _normalised(value: object) -> pathlib.PurePath:
    """Return one input path with redundant separators and components removed."""
    return pathlib.PurePosixPath(str(value))


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
    for key in ("declaration-file", "trusted-record-file"):
        value = _normalised(inputs[key])
        if not _inside(trusted, value):
            msg = f"{key} ({value}) must be inside trusted-directory ({trusted})"
            raise AssertionError(msg)
    download = _normalised(inputs["download-directory"])
    output = _normalised(inputs["output-path"])
    for label, value in (
        ("download-directory", download),
        ("output-path", output),
    ):
        if _overlap(trusted, value):
            msg = (
                f"{label} ({value}) must be disjoint from trusted-directory ({trusted})"
            )
            raise AssertionError(msg)
    if _overlap(download, output):
        msg = f"output-path ({output}) must not overlap download-directory ({download})"
        raise AssertionError(msg)
