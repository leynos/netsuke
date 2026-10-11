"""Run one allowlisted command, bounded or not, with the shell's stream wiring.

The shell wrapped every bounded admission command in GNU ``timeout``, and two
properties of that wrapper are load-bearing rather than incidental:

- a command that outlives its bound is terminated within the bound plus one
  second of grace, and
- no descendant of the command is still running when it is.

GNU ``timeout`` gets the second property by placing the command in a process
group of its own and signalling the group rather than the direct child. Cuprum
spawns without ``start_new_session`` and terminates only the direct child, so a
descendant survives cancellation; worse, a survivor that inherited the captured
pipes keeps them open, so the stream pump never reaches end-of-file and a
two-second bound can overrun by minutes.

This module restores the shell's guarantee. The command is still built through
cuprum -- :func:`cuprum.sh.make` looks the program up in the catalogue and
returns a fully typed argument vector, so the allowlist is what decides which
executables may run -- but it is *spawned* here rather than through
``SafeCmd.run``, because cuprum's ``ExecutionContext`` offers no way to lead a
child into a session of its own and no way to reach the child's process group.
The trade-off is deliberate and confined to this module: the catalogue, the
typed vector, and the result type are cuprum's, and only the spawn and the
escalation are ours.

The streams follow the shell's redirections rather than a uniform choice,
because which of them the gate may lose depends on who reads them:

- A command whose output the gate *parses* has its standard output captured in
  a pipe the gate owns, exactly as a command substitution did. Its standard
  error is never redirected, so it reaches the runner log; only the parsed
  stream is private. That is :func:`run_bounded`'s default, and what the GitHub
  and clock adapters get.
- A command whose output is only *reported* has both streams inherited, as
  :func:`run_bounded`'s ``inherit_streams`` asks for. The ``git`` fetch is the
  one such bounded command, and a sink adapter is the unbounded one.
- A command the shell ran with no bound at all -- the clock adapter -- is
  :func:`run_unbounded`, so an adapter that never returns is waited for rather
  than silently given a bound the shell never placed on it.

A descendant that leaves the group deliberately, with its own ``setsid()``,
still escapes. That hole is GNU ``timeout``'s as well -- the developers' guide
records the same limitation for the Kani scope wrapper -- so it is a
preserved limitation rather than a regression.
"""

import asyncio
import contextlib
import os
import signal
import typing as typ

from cuprum import CommandResult

if typ.TYPE_CHECKING:
    from cuprum import SafeCmd

#: How long the group is given to leave after ``SIGTERM`` before ``SIGKILL``,
#: which is the grace the shell passed to GNU ``timeout`` as ``--kill-after=1s``.
CANCEL_GRACE_SECONDS = 1.0

#: Decoding policy for the stream this module captures itself. It matches
#: cuprum's own default, so a result built here and one built by ``SafeCmd``
#: decode an undecodable byte the same way.
_DECODE_ERRORS = "replace"


async def run_bounded(
    command: SafeCmd,
    *,
    timeout_seconds: int,
    cancel_grace: float = CANCEL_GRACE_SECONDS,
    inherit_streams: bool = False,
) -> CommandResult | None:
    """Run an allowlisted command under a bound that reaches its group.

    Parameters
    ----------
    command
        The allowlisted command to run.
    timeout_seconds
        How long the command may run before its group is signalled.
    cancel_grace
        How long the group is given to leave after ``SIGTERM``.
    inherit_streams
        Whether both of the command's streams go straight to the gate's own.
        The default captures its standard output, which is what a caller that
        *parses* the output needs and what keeps that output private.

    Returns
    -------
    CommandResult | None
        The command's result, or ``None`` when the bound elapsed.

    Notes
    -----
    A command that outlives its bound is sent ``SIGTERM`` as a *group*, which
    is what lets the escalation reach a descendant the command started. The
    direct child is then reaped whether or not it complied, so the bound is
    ``timeout_seconds + cancel_grace`` at worst and no process the command
    started is left running.
    """
    process = await _spawn(command, inherit_streams=inherit_streams)
    if inherit_streams:
        return await _wait_for_exit(command, process, timeout_seconds, cancel_grace)
    return await _wait_for_streams(command, process, timeout_seconds, cancel_grace)


def run_bounded_sync(
    command: SafeCmd,
    *,
    timeout_seconds: int,
    cancel_grace: float = CANCEL_GRACE_SECONDS,
    inherit_streams: bool = False,
) -> CommandResult | None:
    """Drive :func:`run_bounded` to completion, as cuprum's ``run_sync`` does."""
    return asyncio.run(
        run_bounded(
            command,
            timeout_seconds=timeout_seconds,
            cancel_grace=cancel_grace,
            inherit_streams=inherit_streams,
        )
    )


async def run_unbounded(command: SafeCmd) -> CommandResult:
    """Run an allowlisted command with no bound, as the shell read its clock.

    Returns
    -------
    CommandResult
        The command's captured result. The gate's clock adapter was the one
        command the shell left unwrapped, so a clock that never returns hangs
        the step exactly as it did before; giving it a bound here would be a
        behaviour change dressed as a safeguard.
    """
    process = await _spawn(command)
    stdout, stderr = await process.communicate()
    return _result(command, process, stdout, stderr)


def run_unbounded_sync(command: SafeCmd) -> CommandResult:
    """Drive :func:`run_unbounded` to completion, as cuprum's ``run_sync`` does."""
    return asyncio.run(run_unbounded(command))


async def _spawn(
    command: SafeCmd, *, inherit_streams: bool = False
) -> asyncio.subprocess.Process:
    """Start one allowlisted command in a session of its own.

    ``start_new_session`` is the whole point: it gives the command a session
    and process group of its own, so the escalation below can reach everything
    it starts. Without it the group is the gate's own, and signalling the group
    would signal the gate.

    ``inherit_streams`` sends both streams to the gate's own, which is what the
    shell's unredirected command had. The default captures standard output in a
    pipe this process reads, as a command substitution did; standard error is
    inherited either way, because the shell never redirected it.

    Returns
    -------
    asyncio.subprocess.Process
        The started child, with each stream wired as requested.
    """
    return await asyncio.create_subprocess_exec(
        *command.argv_with_program,
        stdout=None if inherit_streams else asyncio.subprocess.PIPE,
        stderr=None,
        start_new_session=True,
    )


async def _wait_for_exit(
    command: SafeCmd,
    process: asyncio.subprocess.Process,
    timeout_seconds: int,
    cancel_grace: float,
) -> CommandResult | None:
    """Bound a command by its own exit, with both streams inherited.

    The wait is on the direct child alone, which is the whole bound here. A
    surviving descendant that inherited the gate's own streams cannot delay it,
    because there is no pipe of ours left to reach end-of-file; the shell was
    bounded the same way, by ``timeout`` waiting on the command it led.

    Returns
    -------
    CommandResult | None
        The command's result, or ``None`` when the bound elapsed.
    """
    exit_waiter = asyncio.ensure_future(process.wait())
    try:
        async with asyncio.timeout(timeout_seconds):
            await asyncio.shield(exit_waiter)
    except TimeoutError:
        await _terminate_group(process, cancel_grace)
        await exit_waiter
        return None
    return _result(command, process, None, None)


async def _wait_for_streams(
    command: SafeCmd,
    process: asyncio.subprocess.Process,
    timeout_seconds: int,
    cancel_grace: float,
) -> CommandResult | None:
    """Bound a command by its exit *and* by its captured standard output.

    The drain is read to end-of-file before the result is built, so the caller
    never parses a prefix of the output. Shielding keeps that drain alive
    across an elapsed bound, so the pipe is still being read while the group is
    being terminated; killing the group closes the last writer, and the same
    task then completes without the bound ever having been interrupted
    mid-read.

    Returns
    -------
    CommandResult | None
        The command's result with both streams captured, or ``None`` when the
        bound elapsed.
    """
    drain = asyncio.ensure_future(process.communicate())
    try:
        async with asyncio.timeout(timeout_seconds):
            stdout, stderr = await asyncio.shield(drain)
    except TimeoutError:
        await _terminate_group(process, cancel_grace)
        stdout, stderr = await drain
        return None
    return _result(command, process, stdout, stderr)


async def _terminate_group(
    process: asyncio.subprocess.Process, cancel_grace: float
) -> None:
    """Terminate a command's whole process group, escalating to ``SIGKILL``.

    The direct child is reaped either way. A group that has already gone -- the
    command exited between the bound elapsing and the signal, or reaped itself
    -- is not a fault, so the lookup failure is suppressed rather than reported.
    """
    _signal_group(process, signal.SIGTERM)
    try:
        async with asyncio.timeout(max(0.0, cancel_grace)):
            await process.wait()
    except TimeoutError:
        _signal_group(process, signal.SIGKILL)
        await process.wait()


def _signal_group(process: asyncio.subprocess.Process, number: signal.Signals) -> None:
    """Send one signal to the command's process group.

    ``start_new_session`` makes the child a session leader, so its process-group
    identifier is its own pid. Signalling the group reaches the child and every
    descendant that inherited it, which signalling the pid alone would not.
    """
    with contextlib.suppress(ProcessLookupError, PermissionError):
        os.killpg(process.pid, number)


def _result(
    command: SafeCmd,
    process: asyncio.subprocess.Process,
    stdout: bytes | None,
    stderr: bytes | None,
) -> CommandResult:
    """Return the cuprum result a bounded command produced.

    Cuprum's :class:`CommandResult` is constructed directly rather than through
    ``SafeCmd.run``, so its shape is what every caller already reads: each
    captured stream decoded, each inherited stream reported as absent, and the
    exit status the child reported.

    Returns
    -------
    CommandResult
        The captured result, with an inherited stream reported as ``None``.
    """
    return CommandResult(
        program=command.program,
        argv=command.argv,
        exit_code=typ.cast("int", process.returncode),
        pid=process.pid,
        stdout=_decoded(stdout),
        stderr=_decoded(stderr),
    )


def _decoded(stream: bytes | None) -> str | None:
    """Decode one captured stream, or report that it was inherited."""
    return None if stream is None else stream.decode("utf-8", _DECODE_ERRORS)
