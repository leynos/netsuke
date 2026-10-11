r"""Translate operating-system outcomes into the gate's own command result.

Every command the gate runs ends in a representation that belongs to somebody
else, and three of them need translating before the policy can classify what
happened:

- A process that dies of a signal has no exit status at all. The shell that
  waited reports ``128`` plus the signal number, where cuprum reports the
  negation of it, so ``kill -KILL`` is ``137`` to a shell and ``-9`` to Python.
- The gate's own bound reports an elapsed timeout as ``None``, where GNU
  ``timeout`` could report ``124`` or ``137``. This port translates ``None`` to
  ``124`` in both cases.
- A launch the operating system refused arrives as an :class:`OSError` rather
  than as a status, where ``timeout`` printed a sentence and exited ``127`` for
  a program that does not exist and ``126`` for any other refusal.

This module is that translation, and it is the only place a shell status
number is interpreted. It sits beside :mod:`commands` and :mod:`delivery`
rather than inside either one, because both adapters need it and neither owns
the other. It imports :mod:`policy` for the vocabulary it produces --
:class:`policy.CommandOutcome` and the :class:`policy.CommandResult` carrying
it -- and for nothing else, so the dependency runs one way: the adapters say
what the operating system did, and the policy decides what it means.

Two of the names below differ by one word and are not interchangeable.
:func:`launch_failure_result` returns the gate's own result, which is what a
caller classifies; :func:`launch_failure_status` returns the bare status, which
is what a caller *reports* -- at the unguarded gate-output write that status
becomes the gate's own exit status, so it has to stay a number.

The launch refusal's sentence is printed here too, because a status alone does
not say which configured program was refused. The shell reached the same state
through GNU ``timeout``, which printed its own sentence, so the wording differs
while the position, the status, and the classification do not.
"""

import enum
import sys
import typing as typ

from . import policy

if typ.TYPE_CHECKING:
    import cuprum


class CommandStatus(enum.IntEnum):
    """Interpret the exit status of a bounded command.

    GNU ``timeout`` reports ``124`` when the command accepted ``SIGTERM`` and
    ``137`` when it had to be killed, so both mean the operation ran out of
    time. Every other status -- including the ``126`` and ``127`` a missing or
    non-executable program produces -- is handed to the caller's own category,
    because a broken adapter is not a timed-out one.

    The bound this port enforces itself reports only :attr:`TIMEOUT_BY_TERM`.
    It has no ``SIGKILL``-then-report path of its own: a group that ignores
    ``SIGTERM`` is escalated to ``SIGKILL`` and that status is discarded, so
    the bound's expiry is reported as ``None`` either way. Leaving
    :attr:`TIMEOUT_BY_KILL` in the set still matters, because a completed
    command independently terminated by ``SIGKILL`` reaches classification
    through :func:`as_bash_status` as ``137``.
    """

    TIMEOUT_BY_TERM = 124
    TIMEOUT_BY_KILL = 137


#: Statuses that mean a bounded command ran out of time rather than failed.
TIMEOUT_STATUSES: frozenset[int] = frozenset(int(status) for status in CommandStatus)


def as_bash_status(exit_code: int) -> int:
    """Translate a reported exit status into the status Bash would have seen.

    A process that dies from a signal does not exit with a status at all; the
    shell that waited reports ``128`` plus the signal number. Python's
    ``subprocess`` family reports the negation of the signal instead, and the
    distinction is not cosmetic: a completed command independently terminated
    by ``SIGKILL`` reports ``-9``, and ``137`` is what
    :attr:`CommandStatus.TIMEOUT_BY_KILL` names. Passing cuprum's ``-9``
    through unchanged would classify that operation as ``api_error`` or
    ``fetch_error`` and silently change the published category.

    The bound this port enforces does not reach classification through a
    signal at all. Its expiry is reported as ``None`` whatever signal ended the
    command, so the escalation to ``SIGKILL`` is never read as a status and the
    elapsed bound is always ``124``; :func:`translate_exit` records that.

    ``143`` is translated too, but it is not a timeout status:
    :data:`TIMEOUT_STATUSES` is exactly ``{124, 137}``, so an operator's
    adapter killed by ``SIGTERM`` keeps its operation's own category.

    A status that is not negative is already the shell's own, including the
    ``124`` the bound reports for itself.

    Returns
    -------
    int
        The status the shell would have reported.

    Examples
    --------
    >>> as_bash_status(-15), as_bash_status(-9), as_bash_status(124)
    (143, 137, 124)
    """
    return 128 + -exit_code if exit_code < 0 else exit_code


def launch_failure_status(error: OSError) -> int:
    """Return the status GNU ``timeout`` reported for a command it could not run.

    Cuprum raises where the shell's ``timeout`` returned a status, so the two
    launch failures have to be mapped back by hand. The mapping matters because
    it is what keeps a missing adapter in ``api_error`` or ``fetch_error``
    rather than forcing it into ``timeout``: only ``124`` and ``137`` mean the
    command ran out of time, and neither of these is either of them.

    Returns
    -------
    int
        ``127`` when the program does not exist, and ``126`` for any other
        reason the operating system refused the launch -- a permission bit, a
        directory, or a failed ``execve``.

    Examples
    --------
    >>> launch_failure_status(FileNotFoundError(2, "No such file or directory"))
    127
    >>> launch_failure_status(PermissionError(13, "Permission denied"))
    126
    """
    return 127 if isinstance(error, FileNotFoundError) else 126


def report_launch_failure(program: str, error: OSError) -> None:
    """Write one diagnostic naming a program the operating system refused.

    The shell reached this state through GNU ``timeout``, which printed its own
    sentence and exited ``126`` or ``127``. Cuprum raises instead, so the port
    prints its own. The wording therefore differs from the shell's; the status
    and the classification do not. Naming the program is the point: it is the
    operator's own adapter configuration, and without it a misconfigured
    ``NETSUKE_RELEASE_ADMISSION_GH_ADAPTER`` would fail every run in silence.
    """
    reason = error.strerror or error.__class__.__name__
    sys.stderr.write(
        f"release-admission adapter could not be run: {program}: {reason}\n"
    )


def launch_failure_result(error: OSError) -> policy.CommandResult:
    """Return the gate's own result for a command it could not launch.

    Returns
    -------
    policy.CommandResult
        A ``launch_failed`` outcome carrying the status GNU ``timeout``
        produced for the same refusal, and no output, because a command that
        never ran printed nothing to parse.

    Notes
    -----
    The outcome is named rather than folded into ``failed`` so the distinction
    stays visible: ``126`` and ``127`` are a fault in the operator's adapter
    configuration rather than in the command, and the status is the number a
    sink's refusal would contribute to the gate's own exit status.

    Examples
    --------
    >>> launch_failure_result(FileNotFoundError(2, "no such file")).status
    127
    """
    return policy.CommandResult(
        outcome=policy.CommandOutcome.LAUNCH_FAILED,
        status=launch_failure_status(error),
        output="",
    )


def translate_exit(
    exit_code: int | None, stdout: str | None = None
) -> policy.CommandResult:
    r"""Translate one finished command into the gate's own result.

    This is the primitive both adapters call, and it takes the two values each
    of them actually holds: an exit status and whatever standard output was
    captured. Cuprum spells the first ``exit_code`` and the standard library
    spells it ``returncode``, so neither spelling is imported here.

    Parameters
    ----------
    exit_code
        The status the command reported, or ``None`` when the gate's bound
        elapsed before it finished.
    stdout
        The captured standard output, or ``None`` when nothing was captured.

    Returns
    -------
    policy.CommandResult
        The outcome, the status in Bash's convention, and the retained output.
        The output is kept only for a command that succeeded, because a failed
        command substitution left Bash's variable empty and the gate must not
        parse a diagnostic as though it were an answer.

    Notes
    -----
    ``None`` is the elapsed bound, which the port reports as ``124``. GNU
    ``timeout`` told ``124`` from ``137`` by whether the command had to be
    killed; the bound here reports ``None`` either way, even after escalating
    to ``SIGKILL``, because that escalation discards the child's own status.
    The elapsed bound is therefore always ``124``, and the distinction GNU
    ``timeout`` drew is not one this port can make.

    A signal death is translated before it is named, because only the shell's
    own ``128 + signal`` spelling matches the timeout statuses: a *completed*
    command independently terminated by ``SIGKILL`` reports ``-9``, and
    untranslated it would be classified as an ordinary failure rather than as a
    timeout.

    Examples
    --------
    >>> translate_exit(None).status
    124
    >>> translate_exit(None).outcome.name
    'TIMED_OUT'
    >>> translate_exit(0, "abc\n").output
    'abc'
    """
    if exit_code is None:
        return policy.CommandResult(
            outcome=policy.CommandOutcome.TIMED_OUT,
            status=int(CommandStatus.TIMEOUT_BY_TERM),
            output="",
        )
    status = as_bash_status(exit_code)
    if status == 0:
        return policy.CommandResult(
            outcome=policy.CommandOutcome.SUCCEEDED,
            status=status,
            output=(stdout or "").rstrip("\n"),
        )
    outcome = (
        policy.CommandOutcome.TIMED_OUT
        if status in TIMEOUT_STATUSES
        else policy.CommandOutcome.FAILED
    )
    return policy.CommandResult(outcome=outcome, status=status, output="")


def translate_result(completed: cuprum.CommandResult | None) -> policy.CommandResult:
    """Translate one command as cuprum reported it.

    Returns
    -------
    policy.CommandResult
        The same result :func:`translate_exit` builds, from cuprum's spelling
        of the two fields.

    Examples
    --------
    >>> translate_result(None).status
    124
    """
    if completed is None:
        return translate_exit(None)
    return translate_exit(completed.exit_code, completed.stdout)


def classify_result(completed: cuprum.CommandResult | None) -> int:
    """Return the status a bounded call contributes to its policy decision.

    This is :func:`translate_result` reduced to the one field a caller that
    only reports a status needs, which is the sink write: the number *is* the
    gate's exit status there, and the status-translation tests pin it directly.

    Returns
    -------
    int
        The command's own status in Bash's convention, or ``124`` when the
        bound elapsed.

    Examples
    --------
    >>> classify_result(None)
    124
    """
    return translate_result(completed).status
