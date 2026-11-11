"""Record how long an upload run spent at each boundary it crossed.

An upload run reaches GitHub three times, and each of them can be the slow or
the failing one: it lists the release, it downloads the assets the listing hid,
and it sends the transfer. A single phase duration for the whole step says the
run took a minute and leaves an operator unable to tell a slow transfer from a
listing that retried. The four numbers here separate them.

A boundary that was not crossed is ``None`` rather than ``0``. "Nothing needed
downloading" and "the download was instantaneous" are different facts, and a
zero would report the first as the second.

Every value is a measured duration in whole milliseconds, or ``null`` for a
boundary the run did not cross; nothing else is recorded. The plan beside this
file is deliberately free of command output for the same reason: an artefact
outlives the run, and a duration is the only thing about a boundary that is
safe to publish.

Every boundary a run crosses is a ``gh`` invocation, so :class:`TimedRunner`
measures them by wrapping the one subprocess seam the uploader already
injects. No call site learns about timing, and a test that supplies a bare
runner still drives the real code and simply records nothing. The runner is
deliberately generic over the seam it wraps: this module has no business
importing ``subprocess`` to describe what a caller's runner returns, and the
type parameter says so instead of forcing a second copy of that alias here.

Writing and reading the record live in
:mod:`release_upload_timing_files`, so a caller that only measures a boundary
imports nothing that can touch a disk.

Run via ``.github/actions/upload-declared-assets``; the action retains the file
beside the plan, and behavioural coverage lives in
``scripts/tests/test_release_upload_timing.py``.
"""

import dataclasses
import time
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

__all__ = [
    "TIMINGS_NAME",
    "BoundaryTimings",
    "TimedRunner",
    "elapsed_ms",
]

#: Name of the file the uploader writes beside its plan.
TIMINGS_NAME = "upload-timings.json"

#: Where the ``gh`` subcommand sits in an argument list: ``gh``, ``release``,
#: then the subcommand that names what is being done with the release.
_SUBCOMMAND_INDEX = 2

#: The subcommand that reads the release, as opposed to transferring to it.
_LIST_SUBCOMMAND = "view"

#: The subcommands that cross a boundary of their own, and which one each is.
#: ``view`` is absent because it is decided by how many listings have happened:
#: the first reconciles, the second describes what the transfer left behind.
_SUBCOMMAND_BOUNDARIES = {"download": "download", "upload": "upload"}


@dataclasses.dataclass(frozen=True, slots=True)
class BoundaryTimings:
    """Hold the elapsed milliseconds one run spent at each boundary.

    Attributes
    ----------
    listing_ms
        The reconciliation listing, taken before any byte moves.
    download_ms
        Downloading the attached assets the listing reported no digest for,
        summed over the assets, or ``None`` when it named none.
    upload_ms
        The transfer, or ``None`` when the run sent nothing -- a dry run, a
        plan with nothing missing, or a refusal.
    verification_ms
        The second listing, taken after the transfer to describe what the
        release holds now. Separate from ``listing_ms`` because it answers a
        different question: not what was already attached, but what arrived.
    """

    listing_ms: int | None = None
    download_ms: int | None = None
    upload_ms: int | None = None
    verification_ms: int | None = None


def elapsed_ms(started_ns: int, ended_ns: int) -> int:
    """Return the whole milliseconds between two monotonic readings.

    Parameters
    ----------
    started_ns
        The reading taken before the boundary was crossed.
    ended_ns
        The reading taken after it.

    Returns
    -------
    int
        The elapsed milliseconds, floored at zero so a clock that steps
        backwards cannot publish a negative duration.

    Examples
    --------
    >>> elapsed_ms(0, 2_500_000)
    2
    >>> elapsed_ms(5, 1)
    0
    """
    return max(0, (ended_ns - started_ns) // 1_000_000)


class TimedRunner[Result]:
    """Wrap a command runner and record how long each boundary took.

    Parameters
    ----------
    inner
        The runner to measure. Production passes the uploader's
        ``PRODUCTION_RUNNER``; a test may pass any stand-in, and the timings
        then describe whatever that stand-in was asked to run.
    clock_ns
        The monotonic reading, injected so a test can make a boundary take a
        known time without sleeping. Defaults to ``time.monotonic_ns``.

    Notes
    -----
    This is the timing seam rather than a set of timed call sites because a
    boundary is exactly one ``gh`` invocation: measuring at the runner means a
    call site added later is measured without knowing anything about telemetry.

    Boundary durations accumulate. A run downloads once per undigested name,
    so ``download_ms`` is the time all of those took; recording only the last
    one would report the cost of one asset as the cost of the boundary.

    A command this class does not recognise as a boundary is passed through
    unmeasured. Attributing an unknown subcommand's duration to one of the four
    boundaries would be inventing a fact.

    Examples
    --------
    >>> import subprocess
    >>> def _ok(_arguments, **_keywords):
    ...     return subprocess.CompletedProcess([], 0, "", "")
    >>> readings = iter([0, 1_000_000, 1_000_000, 5_000_000])
    >>> timed = TimedRunner(_ok, clock_ns=lambda: next(readings))
    >>> _ = timed(["gh", "release", "view", "v1"])
    >>> _ = timed(["gh", "release", "upload", "v1", "a.deb"])
    >>> timed.timings.listing_ms, timed.timings.upload_ms
    (1, 4)
    >>> timed.timings.download_ms is None
    True
    """

    def __init__(
        self,
        inner: cabc.Callable[..., Result],
        *,
        clock_ns: cabc.Callable[[], int] = time.monotonic_ns,
    ) -> None:
        """Hold the runner to measure and the clock to measure it with."""
        self._inner = inner
        self._clock_ns = clock_ns
        self._listings = 0
        self._timings = BoundaryTimings()

    def __call__(self, arguments: cabc.Sequence[str], **keywords: object) -> Result:
        """Run one command, recording its duration under the boundary it is."""
        boundary = self._boundary(arguments)
        if boundary is None:
            return self._inner(arguments, **keywords)
        started_ns = self._clock_ns()
        try:
            return self._inner(arguments, **keywords)
        finally:
            # Recorded even when the command raises: a boundary that failed
            # still took the time it took, and a record missing exactly the
            # slow failure is the one an operator most needs.
            self._record(boundary, elapsed_ms(started_ns, self._clock_ns()))

    @property
    def timings(self) -> BoundaryTimings:
        """The boundaries measured so far.

        Returns
        -------
        BoundaryTimings
            What each crossed boundary took, with ``None`` for the rest.
        """
        return self._timings

    def _boundary(self, arguments: cabc.Sequence[str]) -> str | None:
        """Name the boundary a command is, or ``None`` when it is not one."""
        if not _is_release_command(arguments):
            return None
        subcommand = arguments[_SUBCOMMAND_INDEX]
        if subcommand == _LIST_SUBCOMMAND:
            # The first listing reconciles before any transfer; the second
            # describes what the release holds afterwards.
            self._listings += 1
            return "listing" if self._listings == 1 else "verification"
        return _SUBCOMMAND_BOUNDARIES.get(subcommand)

    def _record(self, boundary: str, milliseconds: int) -> None:
        """Accumulate one boundary's duration across the calls that crossed it."""
        member = f"{boundary}_ms"
        previous = getattr(self._timings, member)
        self._timings = dataclasses.replace(
            self._timings,
            **{member: milliseconds if previous is None else previous + milliseconds},
        )


def _is_release_command(arguments: cabc.Sequence[str]) -> bool:
    """Report whether these arguments are a complete ``gh release`` command.

    Returns
    -------
    bool
        ``True`` when the executable is ``gh``, the command is ``release``,
        and a subcommand follows.

    Examples
    --------
    >>> _is_release_command(["gh", "release", "view", "v1"])
    True
    >>> _is_release_command(["gh", "release"])
    False
    >>> _is_release_command(["python3", "release", "view"])
    False
    """
    return list(arguments[:2]) == ["gh", "release"] and (
        len(arguments) > _SUBCOMMAND_INDEX
    )
