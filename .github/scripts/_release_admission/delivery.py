r"""Deliver one rendered record to a file or to an operator's sink adapter.

A sink is the empty string to append in process, or the name of an adapter to
run. The adapter receives the target path as its only argument and the record
on its standard input, which is the contract an operator's sink script already
implements.

This is the one boundary cuprum cannot express. The shell fed each record to
its adapter with ``printf '%s\n' "$record" | "$adapter" "$file"``, and cuprum
neither passes a stdin to a single command nor gives a pipeline's first stage
anything but ``/dev/null``. The seam is therefore a direct process launch, and
it is confined to this module so the rest of the gate reaches every external
effect through the estate's own machinery.

Both of the adapter's streams are inherited rather than captured, because the
shell redirected neither. A sink's diagnostic therefore reached the runner
exactly as it was printed, and a sink whose output is a report rather than the
record keeps being able to say so.

An adapter the operating system refuses to launch is reported with the status
the shell produced for the same refusal -- ``127`` for a program that does not
exist and ``126`` otherwise -- because at the unguarded gate-output write that
status becomes the gate's own exit status. The refusal is also named on
standard error, with the same sentence the command adapters print, because a
status alone does not say which configured program was refused; the shell's
own ``timeout`` printed such a sentence too. A failed in-process append reports
``1``, which is what the shell's ``>>`` failure produced.
"""

import subprocess  # ruff: ignore[suspicious-subprocess-import] - the sink seam runs one adapter.

from . import policy, statuses


def append_record(sink: str, target: str, record: str) -> int:
    r"""Write one record to a sink, returning the status the write contributed.

    Either path terminates the record with the newline ``printf '%s\n'`` added,
    because both artefacts are JSON Lines and a reader drops the final record
    of a file that omits it.

    Returns
    -------
    int
        ``0`` for a write that succeeded, otherwise the failure status. A sink
        adapter's own status is returned verbatim, because ``set -e`` adopted
        it as the gate's; an in-process append reports ``1``, which is the
        status the shell's own redirection failure produced.

    Notes
    -----
    The record is encoded with ``surrogateescape`` because an environment value
    can carry bytes the locale cannot represent, and an unencodable record must
    cost one record rather than the gate's remaining artefacts.

    The status is kept beside the outcome because at the unguarded gate-output
    write it *becomes* the gate's exit status, so a sink that died of
    ``SIGTERM`` must leave the step exiting ``143``. Translating it into an
    outcome would discard the number the workflow reads.
    """
    payload = record.encode("utf-8", "surrogateescape") + b"\n"
    if not sink:
        return _append_in_process(target, payload)
    return _append_via_adapter(sink, target, payload).status


def _append_in_process(target: str, payload: bytes) -> int:
    r"""Append one record to a file, or refuse as the shell's ``>>`` did.

    An empty target is not a special case with a destination of its own. The
    shell's ``>>"$file"`` with an empty ``file`` failed to open an empty
    filename and reported the failure; it never wrote the record anywhere.
    Reproducing that as "return the refusal status" is therefore the honest
    shape, and inventing a destination would have been a behaviour no shell
    ever exhibited.

    An *unset* target is not the same arm: the shell ran under ``set -u``, so
    its expansion aborted the command with status ``127`` before the
    redirection was attempted at all. The port never sees that state -- every
    target is a ``str`` by the time it is called -- and the statuses agree
    anyway, so the empty case is the one this arm documents.

    Returns
    -------
    int
        ``0`` for a write that succeeded, otherwise ``1`` -- the status the
        shell's own failed redirection produced.

    Notes
    -----
    This opens a file directly rather than through
    :mod:`._release_admission.filesystem`, and deliberately. The port performs
    configuration's two start-up effects on a *fresh* artefact through a
    doubled boundary, while this appends one record to a file that already
    exists and reports the shell's status for a refusal rather than raising a
    diagnostic. The two share an operand convention -- the configured spelling,
    never a resolved :class:`~pathlib.Path` -- and nothing else.
    """
    if not target:
        return 1
    try:
        with open(target, "ab") as stream:  # ruff: ignore[builtin-open] - the shell opened this path by its configured spelling.
            stream.write(payload)
    except OSError:
        return 1
    return 0


def _append_via_adapter(sink: str, target: str, data: bytes) -> policy.CommandResult:
    """Run one sink adapter with the record on its standard input.

    The adapter is the operator's own configured program, named through an
    adapter environment variable. The command is a fixed argument list with no
    shell, and the record arrives on the adapter's standard input rather than
    in its argument vector.

    Returns
    -------
    policy.CommandResult
        The adapter's own outcome and status, with the status in the shell's
        convention, or the launch status the shell reported for an adapter it
        could not run. The output is never retained: a sink's own standard
        output is inherited rather than captured.

    Notes
    -----
    Neither of the adapter's streams is redirected. The shell's
    ``append_record`` ran the adapter as an ordinary command rather than in a
    command substitution, so both of its streams reached the runner; capturing
    either one here would lose an operator's own diagnostic. The record is fed
    straight to the child rather than staged through a pipe the port would have
    to drain.
    """
    try:
        completed = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - a fixed argument list, never a shell.
            [sink, target],
            input=data,
            check=False,
        )
    except OSError as error:
        statuses.report_launch_failure(sink, error)
        return statuses.launch_failure_result(error)
    return statuses.translate_exit(completed.returncode)
