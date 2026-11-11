"""Hold the boundary timing record to what a run actually crossed.

A timings record that reported every boundary as a number would be unable to
say that nothing was downloaded, and one that reported a zero would say the
download was instantaneous. Both are wrong for the same reason, so the cases
here distinguish "not crossed" from "took no time" explicitly.

The durations themselves come from an injected clock. A case that slept to
make a boundary take a millisecond would be slower than the code it tests and
would still not prove the arithmetic.

Run via ``make test-release-publication``.
"""

import json
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the runner seam is under test.
import typing as typ

import pytest
from release_upload_timing import (
    TIMINGS_NAME,
    BoundaryTimings,
    TimedRunner,
    elapsed_ms,
)
from release_upload_timing_files import (
    read_timings,
    record_timings,
    timings_path,
    write_timings,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib


class RefusedListingError(RuntimeError):
    """Report a boundary the stand-in runner raises from.

    A test double's failure need not be the production error it models: the
    claim under test is that the timing wrapper records a boundary which
    raised at all, and a stand-in exception proves that without dragging the
    remote module's vocabulary into a module that no longer imports it.
    """


def _completed(
    arguments: cabc.Sequence[str], **_keywords: object
) -> subprocess.CompletedProcess[str]:
    """Return a successful completion for the recorded arguments."""
    return subprocess.CompletedProcess(list(arguments), 0, '{"assets": []}', "")


def _clock(*readings: int) -> cabc.Callable[[], int]:
    """Return a clock that hands out ``readings`` in order.

    Returns
    -------
    collections.abc.Callable[[], int]
        The injected monotonic reading.
    """
    remaining = iter(readings)
    return lambda: next(remaining)


@pytest.mark.parametrize(
    ("started_ns", "ended_ns", "expected"),
    [
        (0, 2_500_000, 2),
        (0, 999_999, 0),
        (0, 0, 0),
        # A monotonic clock that steps backwards must not publish a negative
        # duration: a reader that summed these would be misled by a value no
        # elapsed time can produce.
        (5, 1, 0),
    ],
)
def test_elapsed_ms_floors_a_duration(
    started_ns: int, ended_ns: int, expected: int
) -> None:
    """Report whole milliseconds, never a negative duration."""
    assert elapsed_ms(started_ns, ended_ns) == expected, (started_ns, ended_ns)


def test_the_timings_are_written_beside_the_plan(tmp_path: pathlib.Path) -> None:
    """Name the record in the plan's own directory, so one retention keeps both."""
    plan = tmp_path / "records" / "upload-plan.json"
    assert timings_path(plan) == tmp_path / "records" / TIMINGS_NAME, timings_path(plan)


def test_an_uncrossed_boundary_round_trips_as_none(tmp_path: pathlib.Path) -> None:
    """Keep "not crossed" distinct from "took no time" through the file."""
    destination = tmp_path / TIMINGS_NAME
    write_timings(destination, BoundaryTimings(listing_ms=4, upload_ms=0))
    read = read_timings(destination)
    assert read.listing_ms == 4, read
    assert read.upload_ms == 0, read
    assert read.download_ms is None, read
    assert read.verification_ms is None, read


def _member_names(text: str) -> list[str]:
    """Return the record's member names in the order the file writes them."""
    pairs = json.loads(text, object_pairs_hook=list)
    return [name for name, _ in pairs]


def test_identical_timings_serialize_identically(tmp_path: pathlib.Path) -> None:
    """Sort the keys, so two runs' records can be diffed as text.

    A job that diffs two attempts' timings is comparing one record's shape with
    another's, so key-order drift would read as a changed run.
    """
    first = tmp_path / "first.json"
    second = tmp_path / "second.json"
    write_timings(first, BoundaryTimings(listing_ms=1, verification_ms=2))
    write_timings(second, BoundaryTimings(listing_ms=1, verification_ms=2))
    assert first.read_bytes() == second.read_bytes(), (
        "identical timings must serialize identically"
    )
    keys = _member_names(first.read_text(encoding="utf-8"))
    assert keys == sorted(keys), "the members must be written in key order"


@pytest.mark.parametrize(
    ("content", "refusal"),
    [
        ("not json", ValueError),
        # A negative duration is a value no elapsed time can produce, while a
        # non-object, a string, or a `true` in a duration's place is the wrong
        # type: whoever corrects the writer needs to know which of the two it
        # has.
        ("[]", TypeError),
        ('"three"', TypeError),
        ('{"listing_ms": -1}', ValueError),
        ('{"listing_ms": true}', TypeError),
        ('{"listing_ms": "3"}', TypeError),
    ],
)
def test_an_unreadable_record_is_refused(
    tmp_path: pathlib.Path, content: str, refusal: type[Exception]
) -> None:
    """Refuse a record a reader could not branch on rather than guess at it."""
    destination = tmp_path / TIMINGS_NAME
    _ = destination.write_text(content, encoding="utf-8")
    with pytest.raises(refusal, match="upload timings"):
        read_timings(destination)


def test_a_recorded_failure_does_not_fail_the_run(
    tmp_path: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Report an unwritable timings destination and leave the exit status alone.

    The timings are a diagnostic; the plan is the contract. Failing the run
    over the diagnostic would replace the uploader's real refusal with a
    message about a full disk.
    """
    plan = tmp_path / "absent" / "upload-plan.json"
    record_timings(plan, BoundaryTimings(listing_ms=2))
    captured = capsys.readouterr()
    assert "upload timings not recorded" in captured.err, captured.err


def test_the_timed_runner_measures_each_boundary(tmp_path: pathlib.Path) -> None:
    """Attribute each ``gh`` invocation's duration to the boundary it is.

    The two listings are the interesting pair: the first reconciles before any
    transfer and the second describes what arrived afterwards, so folding them
    into one number would hide a slow post-upload verification.
    """
    timed = TimedRunner(
        _completed,
        clock_ns=_clock(
            0,
            1_000_000,  # listing
            1_000_000,
            3_000_000,  # download
            3_000_000,
            8_000_000,  # upload
            8_000_000,
            9_500_000,  # verification
        ),
    )
    for arguments in (
        ["gh", "release", "view", "v1", "--json", "assets"],
        ["gh", "release", "download", "v1", "--pattern", "a.deb", "--dir", "d"],
        ["gh", "release", "upload", "v1", "a.deb"],
        ["gh", "release", "view", "v1", "--json", "assets"],
    ):
        _ = timed(arguments)
    timings = timed.timings
    assert timings.listing_ms == 1, timings
    assert timings.download_ms == 2, timings
    assert timings.upload_ms == 5, timings
    assert timings.verification_ms == 1, timings


def test_a_command_that_is_not_a_boundary_is_left_untimed() -> None:
    """Pass an unrecognised command through without inventing a boundary."""
    timed = TimedRunner(_completed, clock_ns=_clock())
    _ = timed(["gh", "auth", "status"])
    _ = timed(["python3", "-c", "pass"])
    assert timed.timings == BoundaryTimings(), timed.timings


def test_a_failing_boundary_still_records_its_duration() -> None:
    """Record a boundary that raised: the slow failure is the one worth seeing."""

    def _fail(_arguments: cabc.Sequence[str], **_keywords: object) -> typ.NoReturn:
        """Refuse the listing, as a failing ``gh`` invocation would."""
        raise RefusedListingError

    timed = TimedRunner(_fail, clock_ns=_clock(0, 4_000_000))
    with pytest.raises(RefusedListingError):
        _ = timed(["gh", "release", "view", "v1"])
    assert timed.timings.listing_ms == 4, timed.timings


def test_the_written_record_names_every_member(tmp_path: pathlib.Path) -> None:
    """Write all four members, so a reader never has to guess at an absence."""
    destination = tmp_path / TIMINGS_NAME
    write_timings(destination, BoundaryTimings())
    decoded = json.loads(destination.read_text(encoding="utf-8"))
    assert set(decoded) == {
        "listing_ms",
        "download_ms",
        "upload_ms",
        "verification_ms",
    }, decoded
