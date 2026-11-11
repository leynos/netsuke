"""Read and write the upload run's boundary timings record.

:mod:`release_upload_timing` owns the record's shape and the arithmetic that
fills it; this module owns its file. The split mirrors
:mod:`release_digest` and :mod:`release_digest_files`: a caller that only needs
to describe or measure a boundary imports nothing that can touch a disk, and
the one place that serialises the record is a place a boundary contract can
name.

The record is an artefact that outlives the run, so what it may contain is
narrowly bounded. Every member is an integer or ``None``, and a value of any
other type is refused rather than coerced: a duration that arrived as a string
or a boolean is a writer that meant something this reader cannot guess, and
guessing would publish a number no boundary measured.

Run via ``.github/actions/upload-declared-assets``; the action retains the file
beside the plan, and behavioural coverage lives in
``scripts/tests/test_release_upload_timing.py``.
"""

import dataclasses
import json
import sys
import typing as typ

import release_upload_timing

if typ.TYPE_CHECKING:
    import pathlib

__all__ = [
    "read_timings",
    "record_timings",
    "timings_path",
    "write_timings",
]

#: The members every written record carries, in the order a reader expects.
_MEMBERS = (
    "listing_ms",
    "download_ms",
    "upload_ms",
    "verification_ms",
)


def timings_path(plan_path: pathlib.Path) -> pathlib.Path:
    """Return where a run writing ``plan_path`` keeps its timings.

    Parameters
    ----------
    plan_path
        The plan destination the caller asked for.

    Returns
    -------
    pathlib.Path
        The timings destination beside it, so a job that retains the plan's
        directory retains both without naming a second location.

    Examples
    --------
    >>> import pathlib
    >>> timings_path(pathlib.Path("artifacts/upload-plan.json"))
    PosixPath('artifacts/upload-timings.json')
    """
    return plan_path.parent / release_upload_timing.TIMINGS_NAME


def write_timings(
    destination: pathlib.Path, timings: release_upload_timing.BoundaryTimings
) -> None:
    """Write ``timings`` as the record a job reads back.

    Parameters
    ----------
    destination
        Path of the JSON record to write.
    timings
        The boundaries this run measured.

    Notes
    -----
    Keys are sorted and the file is newline-terminated so identical runs
    serialize identically, which lets a job diff two of them. An unwritable
    destination propagates ``OSError``; the caller decides whether that is
    worth failing a release over.

    Examples
    --------
    >>> import pathlib, tempfile
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     path = pathlib.Path(tmp) / release_upload_timing.TIMINGS_NAME
    ...     write_timings(path, release_upload_timing.BoundaryTimings(listing_ms=12))
    ...     print(path.read_text(encoding="utf-8"))
    {
      "download_ms": null,
      "listing_ms": 12,
      "upload_ms": null,
      "verification_ms": null
    }
    <BLANKLINE>
    """
    destination.write_text(
        json.dumps(dataclasses.asdict(timings), indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def read_timings(source: pathlib.Path) -> release_upload_timing.BoundaryTimings:
    """Return the record :func:`write_timings` wrote.

    Parameters
    ----------
    source
        Path of the JSON record to read.

    Returns
    -------
    release_upload_timing.BoundaryTimings
        The boundaries the run measured.

    Raises
    ------
    ValueError
        If the file is not JSON, or carries a negative duration.
    TypeError
        If the decoded document is not an object, or a recorded member is
        neither an integer nor ``None``.

    Examples
    --------
    >>> import pathlib, tempfile
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     path = pathlib.Path(tmp) / release_upload_timing.TIMINGS_NAME
    ...     write_timings(path, release_upload_timing.BoundaryTimings(upload_ms=7))
    ...     read_timings(path).upload_ms
    7
    """
    try:
        decoded = json.loads(source.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        msg = f"the upload timings are not JSON: {error}"
        raise ValueError(msg) from error
    if not isinstance(decoded, dict):
        msg = "the upload timings are not a JSON object"
        raise TypeError(msg)
    return release_upload_timing.BoundaryTimings(**{
        member: _milliseconds(decoded.get(member), member=member) for member in _MEMBERS
    })


def _milliseconds(value: object, *, member: str) -> int | None:
    """Return one recorded duration, or refuse a value a duration cannot be.

    Returns
    -------
    int | None
        The recorded milliseconds, or ``None`` for a boundary not crossed.

    Raises
    ------
    TypeError
        If the value is neither ``None`` nor an integer. ``True`` is a
        ``bool`` rather than a count, so it is refused with the same message:
        a JSON ``true`` in a duration's place is a writer that meant something
        other than milliseconds.
    ValueError
        If the value is an integer but negative, which no elapsed time can be.

    Examples
    --------
    >>> _milliseconds(None, member="upload_ms") is None
    True
    >>> _milliseconds(3, member="upload_ms")
    3
    >>> _milliseconds(-1, member="upload_ms")
    Traceback (most recent call last):
        ...
    ValueError: the upload timings member upload_ms is negative
    """
    if value is None:
        return None
    if not isinstance(value, int) or isinstance(value, bool):
        msg = f"the upload timings member {member} is not an integer"
        raise TypeError(msg)
    if value < 0:
        msg = f"the upload timings member {member} is negative"
        raise ValueError(msg)
    return value


def record_timings(
    plan_path: pathlib.Path, timings: release_upload_timing.BoundaryTimings
) -> None:
    """Write ``timings`` beside ``plan_path``, reporting a failure and going on.

    Parameters
    ----------
    plan_path
        The plan destination the caller asked for. The record is written in
        the same directory, so a job that retains one retains both.
    timings
        The boundaries the run measured.

    Notes
    -----
    A run does not fail over this file. The plan is the contract a downstream
    job reads; the timings are a diagnostic, and a release that refused for a
    real reason has already said so on standard error. Turning a full disk
    into an upload failure would replace that diagnostic with a worse one.

    Examples
    --------
    >>> import pathlib, tempfile
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     plan = pathlib.Path(tmp) / "upload-plan.json"
    ...     record_timings(plan, release_upload_timing.BoundaryTimings(download_ms=3))
    ...     read_timings(timings_path(plan)).download_ms
    3
    """
    try:
        write_timings(timings_path(plan_path), timings)
    except OSError as error:
        destination = timings_path(plan_path)
        print(
            f"upload timings not recorded: {destination}: {error}",
            file=sys.stderr,
        )
