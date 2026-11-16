"""Run the extracted binary once to observe the version it reports.

This is the smoke's execution boundary and the only place a downloaded file is
launched. Nothing here decides whether the reported version is the candidate's
— that is :func:`release_consumer_payload.require_probe` — and nothing here
touches the archive; this module passes an absolute path, ``--version`` as an
argument, a wall-clock limit, a ceiling on what it will read back, and a fixed
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

The child is launched through :mod:`subprocess` rather than through ``cuprum``,
which this repository uses for automation scripts generally. The ceiling and
the kill are the reason: ``cuprum`` captures whole, and its line stream is
asynchronous, so bounding the capture there would mean rebuilding this read
against a cancellation path that is further from the signal that has to stop
the child. The smoke also runs under ``uv run --no-project``, where ``cuprum``
is not installed, and this is the only subprocess it starts.

Run through ``scripts/consumer_smoke_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_binstall_probe.py``.
"""

import contextlib
import dataclasses
import subprocess  # ruff: ignore[suspicious-subprocess-import] - probe boundary.
import threading
import typing as typ

from release_binstall_archive import ArchiveRefusedError

if typ.TYPE_CHECKING:
    import pathlib

#: Wall-clock seconds the version probe may run before it is killed.
PROBE_TIMEOUT_SECONDS = 60

#: Largest standard output the probe will read before refusing the banner.
#:
#: A version banner is one short line. This ceiling is far larger than any
#: banner the smoke accepts and far smaller than a buffer worth exhausting, so
#: it bounds the capture without ever refusing a binary that answered.
PROBE_OUTPUT_LIMIT = 8192

#: Environment the probe receives. It inherits nothing, so no credential the
#: runner exports as an environment variable reaches a file this workflow
#: downloaded. It is an allowlist for that one channel, not a sandbox: the
#: child keeps the runner's filesystem, network, and every other resource.
PROBE_ENVIRONMENT = {"PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C"}


class ProbeProcess(typ.Protocol):
    """The slice of a running child the probe drives.

    The surface is kept this small so a test can hand the probe a child that
    writes past the ceiling, or one that never answers, without launching
    anything. Reading is a method rather than an exposed stream because the
    real child's stream is optional at the type level while the probe always
    captures it.
    """

    def read_stdout(self, size: int) -> str:
        """Read at most ``size`` characters, blocking until they arrive."""

    def is_running(self) -> bool:
        """Report whether the child is still alive."""

    def stop(self) -> int:
        """Stop the child, reap it, and report the status it exited with."""


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


@dataclasses.dataclass(frozen=True, slots=True)
class _SubprocessProcess:
    """Adapt :class:`subprocess.Popen` to :class:`ProbeProcess`."""

    process: subprocess.Popen[str]

    def read_stdout(self, size: int) -> str:
        """Read standard output, which the probe always captures as text.

        Returns
        -------
        str
            At most ``size`` characters, decoded.

        Raises
        ------
        ProbeOutputError
            If the launch did not leave a readable stream, which the probe's
            own launch options make unreachable for a real child.
        """
        stream = self.process.stdout
        if stream is None:
            msg = "the shipped binary's standard output was not captured"
            raise ProbeOutputError(msg)
        return stream.read(size)

    def is_running(self) -> bool:
        """Report whether the child is still alive."""
        return self.process.poll() is None

    def stop(self) -> int:
        """Stop the child, reap it, and report the status it exited with.

        Stopping a child that has already exited is a no-op, so the probe uses
        this for the ordinary end of a run as well as for a refusal.

        Returns
        -------
        int
            The status the child exited with.
        """
        self.process.kill()
        return self.process.wait()


def _launch(command: list[str], *, env: dict[str, str]) -> ProbeProcess:
    """Start a real child with a captured, decoded standard output.

    Standard input is closed rather than inherited: a ``--version`` child has
    nothing to read, and leaving the runner's input attached would let it block
    on a stream this smoke never provides.

    Returns
    -------
    ProbeProcess
        The started child, whose standard output the probe reads.
    """
    return _SubprocessProcess(
        subprocess.Popen(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
            command,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            # Standard error goes nowhere rather than into a decoded pipe: a
            # child that writes a non-UTF-8 diagnostic would otherwise fail the
            # launch before the banner that decides acceptance is ever read.
            stderr=subprocess.DEVNULL,
            text=True,
            env=env,
        )
    )


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


def _watchdog(process: ProbeProcess, expired: threading.Event) -> threading.Timer:
    """Start the timer that stops a child outliving the wall-clock budget.

    The timer runs in its own thread because the budget has to expire while the
    probe is blocked reading standard output; killing the child is what ends
    that read, so the timer cannot wait for it to finish first.

    Returns
    -------
    threading.Timer
        The started timer, which the caller cancels once the read returns.
    """

    def stop() -> None:
        """Stop the child unless it has already finished on its own."""
        if not process.is_running():
            return
        expired.set()
        _stopped(process)

    timer = threading.Timer(PROBE_TIMEOUT_SECONDS, stop)
    timer.daemon = True
    timer.start()
    return timer


def _read_banner(process: ProbeProcess) -> str:
    """Read standard output up to the ceiling, stopping a child that passes it.

    One character past the ceiling is requested, so "exactly at the ceiling"
    and "over it" are told apart without reading any further.

    Returns
    -------
    str
        The banner the child wrote, when it fits inside the ceiling.

    Raises
    ------
    ProbeOutputLimitError
        If the child wrote past the ceiling; it is killed before this is
        raised.
    ProbeOutputError
        If the output could not be decoded as UTF-8, or could not be read.
    """
    try:
        banner = process.read_stdout(PROBE_OUTPUT_LIMIT + 1)
    except UnicodeDecodeError as error:
        msg = "the shipped binary wrote bytes that are not UTF-8 to standard output"
        raise ProbeOutputError(msg) from error
    except OSError as error:
        msg = f"the shipped binary's standard output could not be read: {error}"
        raise ProbeOutputError(msg) from error
    if len(banner) <= PROBE_OUTPUT_LIMIT:
        return banner
    _stopped(process)
    msg = (
        f"the shipped binary wrote more than {PROBE_OUTPUT_LIMIT} characters to "
        "standard output and was killed"
    )
    raise ProbeOutputLimitError(msg)


def _reaped(process: ProbeProcess) -> int:
    """Reap the child and return the status it exited with.

    Returns
    -------
    int
        The status the child exited with.

    Raises
    ------
    ProbeLaunchError
        If the child could not be reaped.
    """
    try:
        return process.stop()
    except OSError as error:
        msg = f"the shipped binary could not be reaped: {error}"
        raise ProbeLaunchError(msg) from error


def probe_version(
    binary: pathlib.Path, *, launcher: ProbeLauncher | None = None
) -> ProbeResult:
    r"""Run ``binary --version`` and return its status and standard output.

    Parameters
    ----------
    binary
        Absolute path of the extracted binary.
    launcher
        The callable that starts it. ``None`` starts a real child; a test
        supplies its own so the probe boundary is exercised without executing
        a downloaded file.

    Returns
    -------
    ProbeResult
        The exit status and standard output.

    Raises
    ------
    ProbeTimeoutError
        If the probe exceeds :data:`PROBE_TIMEOUT_SECONDS`.
    ProbeLaunchError
        If the binary could not be started or reaped.
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

    Examples
    --------
    >>> import pathlib
    >>> class Child:
    ...     def read_stdout(self, size):
    ...         return "netsuke 1.2.3\n"
    ...     def is_running(self):
    ...         return False
    ...     def stop(self):
    ...         return 0
    >>> def launcher(command, *, env):
    ...     return Child()
    >>> probe_version(pathlib.Path("/tmp/netsuke"), launcher=launcher).stdout
    'netsuke 1.2.3\n'
    """  # ruff: ignore[docstring-extraneous-exception] - raised by the helpers above
    launch = _launch if launcher is None else launcher
    process = _launched(launch, binary)
    expired = threading.Event()
    watchdog = _watchdog(process, expired)
    try:
        banner = _read_banner(process)
    finally:
        watchdog.cancel()
    status = _reaped(process)
    if expired.is_set():
        msg = (
            f"the shipped binary did not report its version within "
            f"{PROBE_TIMEOUT_SECONDS}s and was killed"
        )
        raise ProbeTimeoutError(msg)
    return ProbeResult(returncode=status, stdout=banner)
