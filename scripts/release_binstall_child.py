"""Start the real child the version probe drives.

The probe decides whether the shipped binary reports the candidate version;
this module owns everything that touches the operating system to ask it. The
seam itself — what the probe may ask of a child, and the refusals those
questions raise — lives in :mod:`release_binstall_probe`, so a test can
substitute a child without the probe importing :mod:`subprocess` at all. What
is left here is the one real implementation and the launch options that make
it safe to refuse one.

The child is started in a session of its own. Stopping it then stops everything
it started, which matters because a descendant that inherited the captured pipe
keeps the probe's read blocked after the child itself has exited; signalling
the child alone would leave that pipe open. Standard input is closed because a
``--version`` child has nothing to read, and standard error is discarded so a
stray diagnostic cannot decide the banner's verdict before it is read.

Run through ``scripts/consumer_smoke_cli.py``; behavioural coverage lives in
``scripts/tests/test_release_binstall_probe.py``.
"""

import os
import signal
import subprocess  # ruff: ignore[suspicious-subprocess-import] - probe boundary.
import time

from release_binstall_probe import ProbeOutputError, ProbeProcess


class _SubprocessProcess:
    """Adapt :class:`subprocess.Popen` to :class:`ProbeProcess`."""

    def __init__(self, process: subprocess.Popen[str]) -> None:
        """Wrap one started child."""
        self.process = process

    def read_stdout(self, size: int) -> str:
        """Read standard output, which the probe always captures as text.

        Returns
        -------
        str
            At most ``size`` characters, decoded.

        Raises
        ------
        ProbeOutputError
            If the launch did not leave a readable stream, which the launch
            options below make unreachable for a real child.
        """
        stream = self.process.stdout
        if stream is None:
            msg = "the shipped binary's standard output was not captured"
            raise ProbeOutputError(msg)
        return stream.read(size)

    def wait_until(self, deadline: float) -> int | None:
        """Wait for the child to exit on its own until ``deadline``.

        Returns
        -------
        int | None
            The status the child exited with, or ``None`` when it was still
            running at ``deadline``. Waiting rather than signalling is what
            keeps a binary that writes its banner, closes its output, and then
            finishes its own cleanup from being killed for having been slow.
        """
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return None
        try:
            return self.process.wait(timeout=remaining)
        except subprocess.TimeoutExpired:
            return None

    def stop(self) -> int:
        """Stop the child, reap it, and report the status it exited with.

        Stopping a child that has already exited is a no-op, so the probe uses
        this only for a refusal; the ordinary end of a run waits instead.

        Returns
        -------
        int
            The status the child exited with.
        """
        _signalled(self.process)
        return self.process.wait()

    def close(self) -> None:
        """Release the captured standard output."""
        stream = self.process.stdout
        if stream is not None:
            stream.close()


def _signalled(process: subprocess.Popen[str]) -> None:
    """Kill the child and every descendant that inherited its streams.

    The child runs in its own session, so its process group holds anything it
    started. Signalling the group is what releases a captured pipe a descendant
    still holds; signalling the child alone would leave that pipe open and the
    probe's read blocked behind it. A platform without process groups falls
    back to killing the child, which is the most it can do.
    """
    kill_group = getattr(os, "killpg", None)
    if kill_group is None:
        process.kill()
        return
    try:
        kill_group(os.getpgid(process.pid), signal.SIGKILL)
    except OSError:
        process.kill()


def launch(command: list[str], *, env: dict[str, str]) -> ProbeProcess:
    """Start a real child with a captured, decoded standard output.

    Parameters
    ----------
    command
        The command and its arguments, already split.
    env
        The environment the child receives, which replaces its initial one.

    Returns
    -------
    ProbeProcess
        The started child, whose standard output the probe reads.

    Examples
    --------
    The child runs with exactly the environment it is given and its standard
    output is read back through the returned object. This example runs the
    interpreter itself rather than a downloaded binary, and passes an empty
    environment to show that the child inherits none of the caller's:

    >>> import sys, time
    >>> child = launch([sys.executable, "-c", "print('netsuke 1.2.3')"], env={})
    >>> child.read_stdout(8192).strip()
    'netsuke 1.2.3'
    >>> _ = child.wait_until(time.monotonic() + 30)
    >>> child.close()
    """
    # Handed to the probe rather than entered as a context manager: the child
    # outlives this call, and the probe closes it on every path it can leave by.
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
            # A session of its own, so a refusal can stop everything the child
            # started and close the pipe a descendant would otherwise hold.
            start_new_session=True,
        )
    )


__all__ = ["launch"]
