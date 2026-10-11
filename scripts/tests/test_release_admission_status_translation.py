r"""Pin the statuses the release-admission gate reports for its own commands.

A subprocess reports a child that died of a signal as the *negation* of that
signal; a shell reports it as ``128 + signal``. The shell gate read every status
through the shell's convention, and two downstream decisions depend on it:

- :func:`policy.classify_command_result` maps only a timed-out outcome to
  ``timeout``, and the outcome is decided by ``statuses`` from the translated
  status, so a ``kill -KILL`` that reached it untranslated (``-9``) would be
  classified as an ordinary command failure and the operation's own category
  would survive a real timeout. ``SIGTERM`` is the contrast: it translates to
  ``143``, which is deliberately *not* a timeout status, so a command that
  accepted the group signal stays an ordinary failure.
- At the unguarded gate-output write, the sink adapter's status *becomes* the
  gate's exit status, so a translated kernel status would leave the step
  exiting a number no shell ever produced.

The launch refusals are the other half. GNU ``timeout`` reported ``127`` for a
program it could not run and ``126`` for any other refusal, and both are
reachable when an operator's adapter variable names a path that is wrong rather
than a program that is absent.

Every case here is a direct call rather than a gate run: the translation is a
pure function of a status, and a run would only interpose cmd-mox between the
test and the value under assertion.

Example (run from the repository root)::

    PYTHONPATH=scripts uv run --no-project --python 3.14 \
        --with pytest==9.0.2 --with 'cuprum==0.1.0' \
        python -m pytest scripts/tests/test_release_admission_status_translation.py \
        -c /dev/null --rootdir=. -p no:cacheprovider
"""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".github" / "scripts"))
import cuprum
from _release_admission import delivery, policy, statuses

#: The statuses a signal death translates to, and the signals they come from.
#: Bash reports ``128 + N``; Python reports ``-N``.
SIGNAL_STATUS_CASES = [
    pytest.param(-15, 143, id="sigterm-is-143"),
    pytest.param(-9, 137, id="sigkill-is-137"),
    pytest.param(-2, 130, id="sigint-is-130"),
]

#: The statuses that are already in the shell's convention and must not move.
ORDINARY_STATUS_CASES = [
    pytest.param(0, 0, id="success-is-unchanged"),
    pytest.param(1, 1, id="failure-is-unchanged"),
    pytest.param(124, 124, id="timeout-is-unchanged"),
    pytest.param(137, 137, id="killed-timeout-is-unchanged"),
    pytest.param(143, 143, id="shell-sigterm-is-unchanged"),
]


@pytest.mark.parametrize(("reported", "expected"), SIGNAL_STATUS_CASES)
def test_signal_statuses_are_translated_into_the_shells_convention(
    reported: int, expected: int
) -> None:
    """Verify a signal death is reported as ``128 + signal``.

    Parameters
    ----------
    reported
        The status a subprocess reports, negated for a signal death.
    expected
        The status a shell reports for the same death.

    Notes
    -----
    ``137`` is the load-bearing case. It is both the shell's ``SIGKILL`` status
    and one of the two statuses :data:`statuses.TIMEOUT_STATUSES` holds, which
    :func:`statuses.translate_exit` reads when it names an outcome, so an
    untranslated ``-9`` would stop the gate recognizing an escalated bound as a
    timeout at all.
    """
    assert statuses.as_bash_status(reported) == expected, (
        "a signal death must be reported in the shell's 128+signal convention"
    )


@pytest.mark.parametrize(("reported", "expected"), ORDINARY_STATUS_CASES)
def test_ordinary_statuses_pass_through_untranslated(
    reported: int, expected: int
) -> None:
    """Verify a status that is already in the shell's convention is unchanged.

    Parameters
    ----------
    reported
        The status the command reported.
    expected
        The status the gate must propagate.

    Notes
    -----
    The translation is a mapping of the negative half-line onto ``128..255``.
    A status that is already positive must not be shifted a second time: ``143``
    from a shell-like child would otherwise become ``271``, which no shell can
    report and which no comparison downstream expects.
    """
    assert statuses.as_bash_status(reported) == expected, (
        "a non-negative status is already in the shell's convention"
    )


@pytest.mark.parametrize(
    ("error", "expected"),
    [
        pytest.param(
            FileNotFoundError(2, "No such file or directory"), 127, id="absent"
        ),
        pytest.param(
            PermissionError(13, "Permission denied"), 126, id="execute-bit-missing"
        ),
        pytest.param(
            NotADirectoryError(20, "Not a directory"), 126, id="not-a-directory"
        ),
    ],
)
def test_launch_refusals_keep_the_shells_statuses(
    error: OSError, expected: int
) -> None:
    """Verify a refused launch reports ``127`` or ``126``, as ``timeout`` did.

    Parameters
    ----------
    error
        The error the operating system raised for the refused launch.
    expected
        The status GNU ``timeout`` reported for the same refusal.

    Notes
    -----
    The distinction is by error class rather than by errno: ``timeout`` printed
    ``127`` when the program could not be found and ``126`` for every other
    refusal, and :class:`FileNotFoundError` is precisely the first of those.
    A path whose parent is a regular file raises :class:`NotADirectoryError`
    (``ENOTDIR``), which ``timeout`` reported as ``126`` -- only ``ENOENT``
    produced ``127``.
    """
    assert statuses.launch_failure_status(error) == expected, (
        "a refused launch must keep the status the shell's timeout produced"
    )


def _result(exit_code: int) -> cuprum.CommandResult:
    """Build the result cuprum would have returned for *exit_code*.

    Returns
    -------
    cuprum.CommandResult
        A completed command with the given status and no captured output.
    """
    program = cuprum.Program("status-translation-probe")
    return cuprum.CommandResult(
        program=program,
        argv=("status-translation-probe",),
        exit_code=exit_code,
        pid=0,
        stdout=None,
        stderr=None,
    )


def test_elapsed_bound_classifies_as_the_timeout_status() -> None:
    """Verify an elapsed bound is classified as a timeout, not as its caller's.

    Notes
    -----
    The bound reports ``None`` rather than a status, so ``translate_exit`` is
    what supplies the ``124`` and, with it, the ``timed_out`` outcome. The
    classification is asserted rather than the literal because the *outcome* is
    what :func:`policy.classify_command_result` reads: a ``None`` that reached
    it as anything else would classify an elapsed bound by the operation's own
    category.
    """
    assert statuses.translate_exit(None).status == 124, (
        "an elapsed bound must carry the timeout status"
    )
    assert statuses.translate_exit(None).outcome == policy.CommandOutcome.TIMED_OUT, (
        "an elapsed bound must be named as a timeout, not as a plain failure"
    )
    classification = policy.classify_command_result(
        statuses.translate_exit(None), policy.ErrorCategory.API_ERROR
    )
    assert classification.outcome == policy.Outcome.FAILURE, (
        "the timeout status must be classified as a failure"
    )
    assert classification.error_category == policy.ErrorCategory.TIMEOUT, (
        "an elapsed bound must reach the timeout category, not the caller's own"
    )


@pytest.mark.parametrize(("reported", "expected"), SIGNAL_STATUS_CASES)
def test_classify_result_translates_a_signalled_command(
    reported: int, expected: int
) -> None:
    """Verify the classification path sees a shell-convention status.

    Parameters
    ----------
    reported
        The status cuprum reports for a signalled command.
    expected
        The status the classification path must see.

    Notes
    -----
    This is the whole point of the translation: ``classify_result`` is the only
    place a completed command's status becomes a decision, so a command killed
    by the escalation must classify as a timeout here rather than fall through
    to the operation's own category.
    """
    assert statuses.classify_result(_result(reported)) == expected, (
        "a signalled command must reach classification in the shell's convention"
    )


def test_classify_result_leaves_an_ordinary_status_alone() -> None:
    """Verify a command that failed on its own terms is not reclassified."""
    assert statuses.classify_result(_result(1)) == 1, (
        "a plain failure must reach classification unchanged"
    )


def _sink(path: Path, body: str, *, executable: bool = True) -> str:
    """Write one sink adapter and return its path as a string.

    Returns
    -------
    str
        The path the gate's sink variable would carry.
    """
    path.write_text(body, encoding="utf-8")
    mode = 0o755 if executable else 0o644
    path.chmod(mode)
    return str(path)


def test_in_process_append_reports_the_shells_redirection_status(
    tmp_path: Path,
) -> None:
    """Verify a failed in-process append reports ``1``, as ``>>`` did.

    Notes
    -----
    Both the empty target and the unwritable target are the shell's own
    ``>>"$file"`` failure, which left Bash reporting ``1``. The empty target is
    not a destination of its own: the shell tried to open an empty filename and
    reported the failure, so nothing was written anywhere.
    """
    directory = tmp_path / "a-directory"
    directory.mkdir()

    assert delivery.append_record("", "", "record") == 1, (
        "an empty append target must report the shell's redirection status"
    )
    assert delivery.append_record("", str(directory), "record") == 1, (
        "an append whose target is a directory must report the same status"
    )


def test_missing_sink_executable_reports_the_shells_status(tmp_path: Path) -> None:
    """Verify a sink program that does not exist reports ``127``."""
    missing = tmp_path / "definitely-not-here"
    target = tmp_path / "target.jsonl"

    assert delivery.append_record(str(missing), str(target), "record") == 127, (
        "a sink the operating system cannot find must report 127, as a shell did"
    )
    assert not target.exists(), "a sink that never ran must not leave a target behind"


def test_unexecutable_sink_reports_the_shells_status(tmp_path: Path) -> None:
    """Verify a sink present but not executable reports ``126``."""
    sink = _sink(tmp_path / "no-execute-bit", "#!/bin/sh\nexit 0\n", executable=False)

    assert delivery.append_record(sink, str(tmp_path / "target.jsonl"), "r") == 126, (
        "a sink the operating system refuses to execute must report 126"
    )


def test_refused_sink_launch_names_the_program(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Verify a sink the operating system refuses is named on standard error.

    Notes
    -----
    The status alone does not say which configured program was refused, and the
    gate's exit status is that status. A mistyped sink variable would otherwise
    fail the step in silence. The shell reached the same state through GNU
    ``timeout``, which printed its own "No such file or directory" line; cuprum
    raises instead, so the port prints its own sentence, naming the program the
    same way the command adapters' diagnostic does.
    """
    missing = tmp_path / "definitely-not-here"

    status = delivery.append_record(str(missing), str(tmp_path / "t.jsonl"), "r")

    assert status == 127, (
        "a program the operating system cannot launch must read as 127"
    )
    captured = capsys.readouterr()
    assert f"release-admission adapter could not be run: {missing}" in captured.err, (
        "a refused sink launch must name the program it could not run"
    )


@pytest.mark.parametrize(
    ("signal_number", "expected"),
    [(15, 143), (9, 137)],
    ids=["sigterm-is-143", "sigkill-is-137"],
)
def test_sink_killed_by_a_signal_reports_the_shells_status(
    tmp_path: Path, signal_number: int, expected: int
) -> None:
    """Verify a sink that dies of a signal reports ``128 + signal``.

    Parameters
    ----------
    tmp_path
        Isolated directory the sink adapter is written into.
    signal_number
        The signal the sink adapter sends to itself.
    expected
        The status the shell reported for the same death.

    Notes
    -----
    At the gate's own output write this status *is* the gate's exit status, so a
    sink that died of a signal must leave the step exiting ``143`` rather than
    ``-15``. A negative exit status is not even representable as a process exit
    status, so the untranslated value would surface as an unexplained error
    rather than as a status the workflow can read.
    """
    sink = _sink(tmp_path / "self-signalling", f"#!/bin/sh\nkill -{signal_number} $$\n")

    assert (
        delivery.append_record(sink, str(tmp_path / "target.jsonl"), "record")
        == expected
    ), "a sink killed by a signal must report the shell's 128+signal status"


def test_sink_delivery_passes_the_record_on_standard_input(tmp_path: Path) -> None:
    """Verify the record reaches the adapter on standard input, not as an argument.

    Notes
    -----
    The adapter contract is ``cat >>"$1"``: one argument naming the target and
    the record on standard input. An adapter that received the record as a
    second argument would satisfy a lazier test while breaking every operator's
    existing script, so the contract is asserted from inside the adapter.
    """
    target = tmp_path / "captured.jsonl"
    sink = _sink(
        tmp_path / "capturing-sink",
        f'#!/bin/sh\ncat >>"{target}"\nprintf "%s" "$#" >>"{target}.argc"\n',
    )

    assert delivery.append_record(sink, str(target), "the record") == 0, (
        "a sink that reads its standard input must be delivered successfully"
    )
    assert target.read_text(encoding="utf-8") == "the record\n", (
        "the adapter must receive the record on its standard input"
    )
    assert (tmp_path / "captured.jsonl.argc").read_text(encoding="utf-8") == "1", (
        "the adapter must receive exactly one argument: its target"
    )
