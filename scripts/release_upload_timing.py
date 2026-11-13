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

import release_upload_boundaries
import release_upload_trace
from release_upload_boundaries import (
    PHASE_CONFIRMATION,
    PHASE_RECONCILIATION,
    UPLOAD_BOUNDARY,
)
from release_upload_trace import CallTrace

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

    def with_measurement(self, boundary: str, milliseconds: int) -> BoundaryTimings:
        """Return these timings with one more crossing of ``boundary`` added.

        Parameters
        ----------
        boundary
            The boundary the call crossed.
        milliseconds
            How long that call took, in whole milliseconds.

        Returns
        -------
        BoundaryTimings
            The new total for that boundary. The receiver is unchanged, so a
            caller holding an earlier reading keeps the numbers it had.

        Notes
        -----
        A boundary is crossed more than once: a run downloads once per
        undigested name, and a retried transfer crosses ``upload`` again.
        Reporting only the last crossing would give one asset's cost as the
        boundary's. An uncrossed boundary stays ``None`` rather than becoming
        ``0``, so "nothing needed downloading" is not published as "the
        download was instantaneous".

        Examples
        --------
        >>> BoundaryTimings().with_measurement("upload", 3).with_measurement(
        ...     "upload", 4
        ... ).upload_ms
        7
        >>> BoundaryTimings().download_ms is None
        True
        """
        member = f"{boundary}_ms"
        previous = getattr(self, member)
        return dataclasses.replace(
            self,
            **{member: milliseconds if previous is None else previous + milliseconds},
        )


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
    wall_clock_ns
        The wall-clock reading a trace record is timestamped with, injected
        for the same reason. Defaults to ``time.time_ns``.
    context
        The run identity each trace record carries, or ``None`` to trace with
        none. Supplied by the caller because this seam does not own process
        state.

    Notes
    -----
    This is the timing seam rather than a set of timed call sites because a
    boundary is exactly one ``gh`` invocation: measuring at the runner means a
    call site added later is measured without knowing anything about telemetry.

    It only *measures*. Which boundary a call belongs to is
    :func:`release_upload_boundaries.boundary_of`'s, the running totals are
    :meth:`BoundaryTimings.with_measurement`'s, and the per-call record is
    :func:`release_upload_trace.record_for_call`'s; each is a separate unit a
    test drives without a subprocess, and this class is the composition of
    them around one call.

    The phase moves from reconciliation to confirmation when this runner *sends
    a transfer*, because that is the event the two listings sit either side of.
    Nothing else has to be announced, and a run that never uploads never
    confirms -- there is no second listing to name.

    A command the classifier does not recognise as a boundary is passed
    through unmeasured. Attributing an unknown subcommand's duration to one of
    the four boundaries would be inventing a fact.

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
        wall_clock_ns: cabc.Callable[[], int] = time.time_ns,
        context: cabc.Mapping[str, str] | None = None,
    ) -> None:
        """Hold the runner to measure, the clocks, and the context to trace with."""
        self._inner = inner
        self._clock_ns = clock_ns
        self._wall_clock_ns = wall_clock_ns
        # A run's trace always starts empty: seeding it would let a caller
        # attribute another run's calls to this one.
        self._trace = CallTrace()
        self._context = dict(context or {})
        self._phase = PHASE_RECONCILIATION
        self._timings = BoundaryTimings()

    def __call__(self, arguments: cabc.Sequence[str], **keywords: object) -> Result:
        """Run one command, recording its duration under the boundary it is."""
        boundary = release_upload_boundaries.boundary_of(arguments, phase=self._phase)
        if boundary is None:
            return self._inner(arguments, **keywords)
        started_ns = self._clock_ns()
        outcome = release_upload_trace.OUTCOME_COMPLETED
        try:
            result = self._inner(arguments, **keywords)
        except BaseException:
            outcome = release_upload_trace.OUTCOME_FAILED
            raise
        else:
            # A command that ran and exited non-zero crossed the boundary and
            # failed at it. Reading only exceptions would trace every refusal
            # ``gh`` reports through its status as a completed call, which is
            # the opposite of what an operator reads the trace for.
            if _failed(result):
                outcome = release_upload_trace.OUTCOME_FAILED
            return result
        finally:
            # Recorded even when the command raises: a boundary that failed
            # still took the time it took, and a record missing exactly the
            # slow failure is the one an operator most needs. One clock reading
            # serves both records, so the two cannot disagree about it.
            ended_ns = self._clock_ns()
            self._timings = self._timings.with_measurement(
                boundary, elapsed_ms(started_ns, ended_ns)
            )
            if boundary == UPLOAD_BOUNDARY:
                # A transfer turns the question this run asks of the release;
                # the phase is derived from the call, not announced by a
                # caller who might forget.
                self._phase = PHASE_CONFIRMATION
            self._trace = self._trace.appended(
                release_upload_trace.record_for_call(
                    boundary=boundary,
                    outcome=outcome,
                    readings=release_upload_trace.CallReadings(
                        started_ns=started_ns,
                        ended_ns=ended_ns,
                        timestamp_ns=self._wall_clock_ns(),
                    ),
                    context=self._context,
                )
            )

    @property
    def timings(self) -> BoundaryTimings:
        """The boundaries measured so far.

        Returns
        -------
        BoundaryTimings
            What each crossed boundary took, with ``None`` for the rest.
        """
        return self._timings

    @property
    def trace(self) -> CallTrace:
        """The per-call records this runner has produced.

        Returns
        -------
        CallTrace
            One record for each traced call, in the order they finished.
        """
        return self._trace


#: The attribute a completed command reports its exit status through. Read
#: structurally rather than by importing ``subprocess``: this module wraps a
#: seam, and the caller may supply any stand-in whose result carries a status.
_STATUS_ATTRIBUTE = "returncode"


def _failed(result: object) -> bool:
    """Report whether a returned result describes a call that failed.

    Returns
    -------
    bool
        ``True`` when the result carries a non-zero status. A result with no
        status at all, or one of another type, is left to the caller: this
        function only recognises the shape it can read.

    Examples
    --------
    >>> import subprocess
    >>> _failed(subprocess.CompletedProcess([], 1, "", ""))
    True
    >>> _failed(subprocess.CompletedProcess([], 0, "", ""))
    False
    >>> _failed("no status here")
    False
    """
    status = getattr(result, _STATUS_ATTRIBUTE, 0)
    return isinstance(status, int) and status != 0
