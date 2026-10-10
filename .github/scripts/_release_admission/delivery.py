r"""Deliver one rendered record to a file, an adapter, or standard output.

A sink is the empty string to append in process, or the name of an adapter to
run. The adapter receives the target path as its only argument and the record
on its standard input, which is the contract an operator's sink script already
implements.

This is the one boundary cuprum cannot express. The shell fed each record to
its adapter with ``printf '%s\\n' "$record" | "$adapter" "$file"``, and cuprum
neither passes a stdin to a single command nor gives a pipeline's first stage
anything but ``/dev/null``. The seam is therefore a direct process launch, and
it is confined to this module so the rest of the gate reaches every external
effect through the estate's own machinery.
"""

import subprocess  # ruff: ignore[suspicious-subprocess-import] - the sink seam runs one adapter.
import typing as typ

if typ.TYPE_CHECKING:
    from pathlib import Path


def append_record(sink: str, target: Path | None, record: str) -> int:
    r"""Write one record to a sink, returning the status the write contributed.

    Either path terminates the record with the newline ``printf '%s\\n'`` added,
    because both artefacts are JSON Lines and a reader drops the final record
    of a file that omits it.

    Returns
    -------
    int
        ``0`` for a write that succeeded, otherwise the failure status. A sink
        adapter's own status is returned verbatim, because ``set -e`` adopted
        it as the gate's; a failed in-process append reports ``1``, which is
        the status the shell's ``printf`` failure would have produced.

    Notes
    -----
    A ``None`` target is the state a required variable left behind before the
    gate created its artefacts. The shell wrote the record to its own standard
    output in that state, because the path it had substituted nothing into the
    append redirection; this reproduces that rather than discarding the record.

    The record is encoded with ``surrogateescape`` because an environment value
    can carry bytes the locale cannot represent, and an unencodable record must
    cost one record rather than the gate's remaining artefacts.
    """
    payload = record.encode("utf-8", "surrogateescape") + b"\n"
    if not sink:
        return _append_in_process(target, payload)
    return _append_via_adapter(sink, target, payload)


def _append_in_process(target: Path | None, payload: bytes) -> int:
    """Append one record to a file, or to standard output when it is unset."""
    if target is None:
        return _append_to_standard_output(payload)
    try:
        with target.open("ab") as stream:
            stream.write(payload)
    except OSError:
        return 1
    return 0


def _append_to_standard_output(payload: bytes) -> int:
    """Complete one append that had no usable target by writing fd 1.

    ``cat`` is the system's own copier and no sink exists in this state: the
    shell wrote the record to its standard output because the append
    redirection had no operand to open. An absent ``cat`` therefore costs one
    record, not the gate, exactly as the shell's failed redirection did.

    Returns
    -------
    int
        ``0`` for a write that succeeded, otherwise ``1``.
    """
    try:
        completed = subprocess.run(
            ["cat"],  # ruff: ignore[start-process-with-partial-path] - resolved from the fixed PATH.
            input=payload,
            check=False,
        )
    except OSError:
        return 1
    return completed.returncode


def _append_via_adapter(sink: str, target: Path | None, data: bytes) -> int:
    """Run one sink adapter with the record on its standard input.

    The adapter is the operator's own configured program, named through an
    adapter environment variable, and its status is returned verbatim. The
    command is a fixed argument list with no shell, and the record arrives on
    the adapter's standard input rather than in its argument vector.

    Returns
    -------
    int
        The adapter's own exit status, or ``1`` when it could not be run.
    """
    try:
        completed = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - a fixed argument list, never a shell.
            [sink, _rendered_target(target)],
            input=data,
            capture_output=True,
            check=False,
        )
    except OSError:
        return 1
    return completed.returncode


def _rendered_target(target: Path | None) -> str:
    """Render an append target as the path the shell would have passed."""
    return "" if target is None else str(target)
