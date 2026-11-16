"""Run the extracted binary once to observe the version it reports.

This is the smoke's execution boundary and the only place a downloaded file is
launched. Nothing here decides whether the reported version is the candidate's
— that is :func:`release_consumer_payload.require_probe` — and nothing here
touches the archive; this module passes an absolute path, ``--version`` as an
argument, a wall-clock budget, a ceiling on what it will read back, and a fixed
environment, then reports what the child did.

The environment is fixed rather than inherited. The job holds a read-only
token, but the binary came from a public URL and has no business reading any
credential the runner happens to export. The launcher is injected so a test can
observe the launch without executing a downloaded file.

Standard error is discarded rather than captured. Only the banner decides
acceptance, and decoding a stray byte on standard error would let a diagnostic
the smoke never reads turn a correct banner into a failed run.

Standard output is read to a ceiling rather than captured whole, and a child
that writes past it is killed rather than read further. A banner is one short
line, so the ceiling never refuses a binary that answered the question it was
asked, and the read is incremental so the bytes a hostile child writes never
become the smoke's memory.

The read runs on its own daemon thread and is joined against one deadline,
because a blocking read cannot be bounded any other way: the child is not the
only holder of its standard output, so a descendant that inherited the pipe
keeps the read blocked after the child itself has exited. The child is started
in its own session, so stopping it stops everything it started and releases the
pipe that would otherwise stay open. The same deadline then bounds the wait for
the child to exit: a binary that closes its standard output and then hangs is
refused as a timeout rather than waited on, and one that closes it and finishes
is reaped normally rather than killed for having been slow to exit. Every
refusal stops, reaps, and closes the child, so no path leaves a process behind.

The child is launched through :mod:`subprocess` rather than through ``cuprum``,
which this repository uses for automation scripts generally. The ceiling and
the kill are the reason: ``cuprum`` captures whole, and its line stream is
asynchronous, so bounding the capture there would mean rebuilding this read
against a cancellation path that is further from the signal that has to stop
the child. The smoke also runs under ``uv run --no-project``, where ``cuprum``
is not installed, and this is the only subprocess it starts. The real child
lives in :mod:`release_binstall_child`, so this module holds the seam and the
decisions rather than the process plumbing.

Run through ``scripts/consumer_smoke_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_binstall_probe.py``.
"""

import contextlib
import dataclasses
import threading
import time
import typing as typ

from release_binstall_archive import ArchiveRefusedError

if typ.TYPE_CHECKING:
    import pathlib

#: Wall-clock seconds the version probe may run before it is killed.
#:
#: The budget covers reading the banner and waiting for the child to exit, so a
#: child cannot extend its run by closing its standard output and staying alive.
PROBE_TIMEOUT_SECONDS = 60

#: Largest standard output the probe will read before refusing the banner.
#:
#: A version banner is one short line. This ceiling is far larger than any
#: banner the smoke accepts and far smaller than a buffer worth exhausting, so
#: it bounds the capture without ever refusing a binary that answered.
PROBE_OUTPUT_LIMIT = 8192

#: Seconds granted to the reader thread after the deadline stopped the child.
#:
#: Killing the child's process group closes the write end of the pipe, so the
#: blocked read returns at once; this only bounds the wait for the thread to
#: observe that, and it is not part of the probe's own budget.
PROBE_DRAIN_SECONDS = 5

#: Environment the probe receives. It inherits nothing, so no credential the
#: runner exports as an environment variable reaches a file this workflow
#: downloaded. It is an allowlist for that one channel, not a sandbox: the
#: child keeps the runner's filesystem, network, and every other resource.
PROBE_ENVIRONMENT = {"PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C"}


class ProbeProcess(typ.Protocol):
    """The slice of a running child the probe drives.

    The surface is kept this small so a test can hand the probe a child that
    writes past the ceiling, one that never answers, or one that closes its
    output and then lingers, without launching anything. Reading is a method
    rather than an exposed stream because the real child's stream is optional
    at the type level while the probe always captures it.
    """

    def read_stdout(self, size: int) -> str:
        """Read at most ``size`` characters, blocking until they arrive."""

    def wait_until(self, deadline: float) -> int | None:
        """Wait for the child to exit, reporting ``None`` past ``deadline``."""

    def stop(self) -> int:
        """Stop the child, reap it, and report the status it exited with."""

    def close(self) -> None:
        """Release the captured standard output."""


class ProbeLauncher(typ.Protocol):
    """Start the child whose standard output the probe will read."""

    def __call__(self, command: list[str], *, env: dict[str, str]) -> ProbeProcess:
        """Start ``command`` with exactly ``env`` as its environment."""


class ProbeLaunchError(ArchiveRefusedError):
    """Report a shipped binary that could not be launched at all."""


class ProbeTimeoutError(ArchiveRefusedError):
    """Report a shipped binary that did not answer within the time budget."""


class ProbeOutputError(ArchiveRefusedError):
    """Report standard output the probe could not read as text.

    The banner comparison needs text, and a replaced or ignored byte would
    hide the fact that the binary wrote something this smoke cannot read. The
    refusal names that fact instead of presenting a mangled banner as if the
    binary had printed it.
    """


class ProbeOutputLimitError(ArchiveRefusedError):
    """Report standard output longer than the probe will read.

    The child is killed before this is raised, so a binary that floods its
    standard output cannot outlive the refusal it caused.
    """


@dataclasses.dataclass(frozen=True, slots=True)
class ProbeResult:
    """Hold the exit status and standard output of one ``--version`` probe."""

    returncode: int
    stdout: str


def _launched(launch: ProbeLauncher, binary: pathlib.Path) -> ProbeProcess:
    """Start the child, translating a launch that never happened.

    Returns
    -------
    ProbeProcess
        The started child, whose standard output the probe reads.

    Raises
    ------
    ProbeLaunchError
        If the child could not be started at all.
    """
    try:
        return launch([str(binary), "--version"], env=PROBE_ENVIRONMENT)
    except OSError as error:
        msg = f"the shipped binary could not be launched: {error}"
        raise ProbeLaunchError(msg) from error


def _stopped(process: ProbeProcess) -> None:
    """Stop the child and reap it, so no refusal leaves a process behind."""
    # A child that is already gone raises here. There is nothing left to reap,
    # and the refusal that prompted the stop is the outcome the caller reports.
    with contextlib.suppress(OSError):
        _ = process.stop()


@dataclasses.dataclass(slots=True)
class _Reading:
    """Hold what the reader thread observed, for the caller to act on.

    Attributes
    ----------
    size
        The number of characters to ask for: one past the ceiling, so
        "exactly at the ceiling" and "over it" are told apart without reading
        any further.
    banner
        What the child wrote, once the read returned.
    error
        What the read raised instead of returning, if it did.
    """

    size: int
    banner: str | None = None
    error: BaseException | None = None

    def run(self, process: ProbeProcess) -> None:
        """Read the child's standard output, recording the outcome."""
        try:
            self.banner = process.read_stdout(self.size)
        except (UnicodeDecodeError, OSError, ValueError) as error:
            # ValueError covers a stream the probe closed from the main thread
            # after the drain budget expired. It is a refusal to report, not a
            # traceback to leave on a daemon thread nobody is watching.
            self.error = error


def _reader(process: ProbeProcess, reading: _Reading) -> threading.Thread:
    """Start the daemon thread that performs the blocking read.

    Returns
    -------
    threading.Thread
        The started reader. The caller joins it against a deadline, because a
        blocked read cannot be interrupted by anything but end of file.
    """
    reader = threading.Thread(target=reading.run, args=(process,), daemon=True)
    reader.start()
    return reader


def _drained(reader: threading.Thread) -> None:
    """Wait briefly for a reader the stopped child has released."""
    reader.join(PROBE_DRAIN_SECONDS)


def _unreadable_message(error: BaseException | None) -> str:
    """Diagnose standard output the probe could not read as a banner.

    Returns
    -------
    str
        The diagnosis naming why no banner could be read. An ``error`` of
        ``None`` means the read neither returned nor raised, which is a
        captured stream this probe cannot use at all; reporting it as empty
        output would blame the binary for a fault in the adapter.
    """
    if isinstance(error, UnicodeDecodeError):
        return "the shipped binary wrote bytes that are not UTF-8 to standard output"
    if error is None:
        return "the shipped binary's standard output could not be read"
    return f"the shipped binary's standard output could not be read: {error}"


def _banner(process: ProbeProcess, deadline: float) -> str:
    """Read standard output up to the ceiling, bounded by ``deadline``.

    Returns
    -------
    str
        The banner the child wrote, when it fits inside the ceiling.

    Raises
    ------
    ProbeTimeoutError
        If the read has not returned by ``deadline``; the child's process group
        is stopped before this is raised, which is what releases the pipe.
    ProbeOutputError
        If the output could not be decoded as UTF-8, or could not be read.
    ProbeOutputLimitError
        If the child wrote past the ceiling; it is stopped before this is
        raised.
    """
    reading = _Reading(size=PROBE_OUTPUT_LIMIT + 1)
    reader = _reader(process, reading)
    reader.join(max(0.0, deadline - time.monotonic()))
    if reader.is_alive():
        _stopped(process)
        _drained(reader)
        msg = (
            f"the shipped binary did not report its version within "
            f"{PROBE_TIMEOUT_SECONDS}s and was killed"
        )
        raise ProbeTimeoutError(msg)
    if reading.banner is None:
        _stopped(process)
        raise ProbeOutputError(_unreadable_message(reading.error)) from reading.error
    if len(reading.banner) <= PROBE_OUTPUT_LIMIT:
        return reading.banner
    _stopped(process)
    msg = (
        f"the shipped binary wrote more than {PROBE_OUTPUT_LIMIT} characters to "
        "standard output and was killed"
    )
    raise ProbeOutputLimitError(msg)


def _observed(process: ProbeProcess, deadline: float) -> tuple[str, int]:
    """Read the banner, then wait for the child to exit within the budget.

    Returns
    -------
    tuple[str, int]
        The banner and the status the child exited with.

    Raises
    ------
    ProbeTimeoutError
        If the read or the wait has not completed by ``deadline``.
    """
    banner = _banner(process, deadline)
    status = process.wait_until(deadline)
    if status is None:
        _stopped(process)
        msg = (
            f"the shipped binary did not report its version within "
            f"{PROBE_TIMEOUT_SECONDS}s and was killed"
        )
        raise ProbeTimeoutError(msg)
    return banner, status


def probe_version(binary: pathlib.Path, *, launcher: ProbeLauncher) -> ProbeResult:
    r"""Run ``binary --version`` and return its status and standard output.

    Parameters
    ----------
    binary
        Absolute path of the extracted binary.
    launcher
        The callable that starts it. A test supplies its own so the probe
        boundary is exercised without executing a downloaded file; the CLI
        supplies :func:`release_binstall_child.launch`.

    Returns
    -------
    ProbeResult
        The exit status and standard output.

    Raises
    ------
    ProbeTimeoutError
        If the probe exceeds :data:`PROBE_TIMEOUT_SECONDS`.
    ProbeLaunchError
        If the binary could not be started.
    ProbeOutputError
        If standard output is not UTF-8 text, so no banner can be read from it.
    ProbeOutputLimitError
        If standard output is longer than :data:`PROBE_OUTPUT_LIMIT`; the child
        is killed before this is raised.

    Notes
    -----
    A non-zero exit status is not a failure here. It is reported in the
    result so the banner decision can name the exit status in its diagnosis,
    which is a different operational fact from a binary that never ran.

    Every refusal stops and reaps the child, and the captured standard output
    is released on the way out whether the run was accepted or refused.

    Examples
    --------
    >>> import pathlib
    >>> class Child:
    ...     def read_stdout(self, size):
    ...         return "netsuke 1.2.3\n"
    ...     def wait_until(self, deadline):
    ...         return 0
    ...     def stop(self):
    ...         return 0
    ...     def close(self):
    ...         pass
    >>> def launcher(command, *, env):
    ...     return Child()
    >>> probe_version(pathlib.Path("/tmp/netsuke"), launcher=launcher).stdout
    'netsuke 1.2.3\n'
    """  # ruff: ignore[docstring-extraneous-exception] - raised by the helpers above
    process = _launched(launcher, binary)
    deadline = time.monotonic() + PROBE_TIMEOUT_SECONDS
    try:
        banner, status = _observed(process, deadline)
    finally:
        # Closing after the child is gone cannot disturb a live reader, and a
        # stream that is already closed raises here for no reason worth
        # reporting.
        with contextlib.suppress(OSError, ValueError):
            process.close()
    return ProbeResult(returncode=status, stdout=banner)
