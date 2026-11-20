r"""Pin the wall-clock reading that decides whether evidence is still fresh.

Freshness is a comparison between a moment recorded in the manifest and the
moment the gate asks the question, so the reading is an input to a security
decision rather than a diagnostic. The reading must therefore be reported when
it cannot be used. Replacing it with zero would refuse every document as
ancient, and *keeping* a non-finite value would be worse: ``nan`` compares
false with everything, so a document judged against it would pass every bound
the gate publishes. ``inf`` and ``-inf`` read as a document from the far future
and one from the beginning of time.

Two layers are pinned here. :func:`commands.read_wall_clock` decides what a
reading *is*, and the gate decides what an unusable reading *means*: an
operation stops with ``failure``/``unknown`` at whichever of the two freshness
questions it was asked at. The clock is read twice in a run that admits fresh
evidence -- once when ``check_scan_freshness`` first observes the document and
again when ``verify_evidence`` is about to admit on it -- so both ends are
exercised, rather than the first alone.

The unit cases drive a stand-in clock of their own instead of a cmd-mox double,
because the classification belongs to the reading rather than to the gate: the
answer is one small program whose status and standard output the case chooses.
The end-to-end cases then show that each unusable answer reaches a refusal, so
a gate that started treating an unusable reading as a moment fails here rather
than admitting an unbounded document.

Run via ``make test-release-admission``.
"""

import itertools
import typing as typ

import pytest
from _release_admission import commands
from release_admission_evidence_fixtures import FRESH
from release_admission_test_scenarios import fail_nth_call
from release_admission_test_support import (
    CANARY_BY_OPERATION,
    METRICS_VALIDATOR,
    assert_failure_trace_sequence,
    expected_operation_labels,
    install_adapted,
    operation_duration,
    operation_records,
    run_gate,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

    from cmd_mox import CmdMox
    from cmd_mox.ipc import Invocation
    from release_admission_test_doubles import Handler, Wrapper

pytest_plugins = ("cmd_mox.pytest_plugin",)

#: The diagnostic the doubled clock prints when a case makes it refuse.
CLOCK_DIAGNOSTIC = "the clock failed\n"

#: The clock invocations, counting from one, that read the *wall* clock in a
#: run that admits fresh evidence. Each operation is bounded by two monotonic
#: readings, and the two evidence questions ask the wall clock once each inside
#: their operation, which puts those readings eighth and eleventh.
FRESHNESS_CLOCK_READ = 8
ADMISSION_CLOCK_READ = 11

#: The last clock invocation of that run: the monotonic reading that closes
#: ``verify_evidence`` after its admission decision was taken.
FINAL_CLOCK_READ = 12


def _clock_stub(directory: Path, body: str) -> str:
    """Write an executable stand-in clock and return its path.

    The gate runs its clock adapter with ``-c`` and a program of its own, so a
    stand-in only has to ignore its arguments and answer with the status and
    output the case chooses.

    Parameters
    ----------
    directory
        Directory to write the stand-in into.
    body
        The shell commands the stand-in runs.

    Returns
    -------
    str
        The absolute path the case configures as the clock adapter.
    """
    program = directory / "clock"
    program.write_text(f"#!/bin/sh\n{body}\n", encoding="utf-8")
    program.chmod(0o755)
    return str(program)


def _read_with(clock: str) -> tuple[float, bool]:
    """Read the wall clock through *clock*, as the gate's adapter does.

    Returns
    -------
    tuple[float, bool]
        The reading in seconds and whether it is unusable.
    """
    return commands.read_wall_clock(commands.Programs(gh="gh", git="git", clock=clock))


@pytest.mark.parametrize(
    ("body", "expected"),
    [
        pytest.param(
            "printf '%s\\n' 'the clock failed' >&2\nexit 3",
            (0.0, True),
            id="refused",
        ),
        pytest.param("printf '%s\\n' 'not-a-number'", (0.0, True), id="not-a-number"),
        pytest.param("printf '%s\\n' 'nan'", (0.0, True), id="nan"),
        pytest.param("printf '%s\\n' 'inf'", (0.0, True), id="inf"),
        pytest.param("printf '%s\\n' '-inf'", (0.0, True), id="negative-inf"),
        pytest.param("printf '%s\\n' ''", (0.0, True), id="empty"),
        pytest.param("printf '%s\\n' '1.5'", (1.5, False), id="fraction"),
        pytest.param("printf '%s\\n' '0'", (0.0, False), id="epoch"),
        pytest.param("printf '%s\\n' '-2.25'", (-2.25, False), id="negative"),
        pytest.param("printf '%s\\n' '1e3'", (1000.0, False), id="exponent"),
    ],
)
def test_the_clock_reading_is_usable_exactly_when_it_is_a_finite_number(
    tmp_path: Path,
    body: str,
    expected: tuple[float, bool],
) -> None:
    """Require an unusable reading to be flagged rather than replaced.

    Zero is a real instant, so the flag is what separates a reading of the
    epoch from a reading the gate could not take. A finite value therefore
    travels with its own sign and magnitude, including zero and a moment
    before the epoch, while a refused read, a word, and both non-finite values
    travel with the zero fallback the caller must never treat as a moment.

    Parameters
    ----------
    tmp_path
        Directory the stand-in clock is written into.
    body
        The shell commands the stand-in runs.
    expected
        The reading and unusable flag the gate must report.
    """
    assert _read_with(_clock_stub(tmp_path, body)) == expected, (
        f"the stand-in clock answered {body!r}, so the gate must report "
        f"{expected!r} rather than substitute a moment it did not observe"
    )


def answer_on_nth_call(number: int, text: str) -> Wrapper:
    """Answer one numbered clock invocation with *text* and a zero status.

    A clock that *succeeds* while printing something unusable is a different
    fault from one that refuses, and the wall clock is where the difference
    matters: a non-number is a successful read of the wall clock that the
    freshness decision must still refuse. The status is zero so the case
    isolates the reading from the adapter's own failure.

    Returns
    -------
    Wrapper
        A wrapper that answers one numbered invocation with *text*.
    """
    remaining = itertools.count(1)

    def wrapper(invocation: Invocation, handler: Handler) -> tuple[str, str, int]:
        """Answer the selected invocation, and delegate every other."""
        if next(remaining) == number:
            return text, "", 0
        return handler(invocation)

    return wrapper


def _refused(number: int) -> Wrapper:
    """Make the numbered clock invocation fail, as a broken adapter does."""
    return fail_nth_call(number, CLOCK_DIAGNOSTIC)


def _answering(text: str) -> cabc.Callable[[int], Wrapper]:
    """Return a factory that makes one clock invocation print *text*.

    Returns
    -------
    cabc.Callable[[int], Wrapper]
        A factory taking the invocation number the case spoils.
    """

    def make(number: int) -> Wrapper:
        """Answer the numbered invocation with the captured text."""
        return answer_on_nth_call(number, text)

    return make


#: The answers a clock adapter can give that are not a moment. ``nan`` is the
#: one that must never be mistaken for a usable reading: every comparison
#: against it is false, so a window test that accepted it would admit any
#: document at all.
UNUSABLE_ANSWERS = (
    pytest.param(_refused, id="refused"),
    pytest.param(_answering("not-a-number\n"), id="not-a-number"),
    pytest.param(_answering("nan\n"), id="nan"),
    pytest.param(_answering("inf\n"), id="inf"),
)


@pytest.mark.parametrize("answer", UNUSABLE_ANSWERS)
@pytest.mark.parametrize(
    "question",
    [
        pytest.param((FRESHNESS_CLOCK_READ, "check_scan_freshness"), id="freshness"),
        pytest.param((ADMISSION_CLOCK_READ, "verify_evidence"), id="admission"),
    ],
)
def test_an_unusable_wall_clock_reading_refuses_admission(
    cmd_mox: CmdMox,
    tmp_path: Path,
    answer: cabc.Callable[[int], Wrapper],
    question: tuple[int, str],
) -> None:
    """Require both freshness questions to refuse an unusable reading.

    The document is fresh, so the gate would admit it if the reading were
    replaced by any number. Requiring the refusal at each question is what
    separates "the gate noticed a broken clock" from "the gate admitted on an
    instant it never observed". The failing operation's closing clock reading
    still happens -- it is what publishes the operation record -- and nothing
    after it does, which is why the run stops one invocation later.

    Parameters
    ----------
    cmd_mox
        Active controller whose shims stand in for every adapter.
    tmp_path
        Isolated output directory for the gate.
    answer
        The factory that spoils one numbered clock invocation.
    question
        The wall-clock invocation the case spoils, and the operation whose
        freshness question that reading answers.
    """
    read_number, operation = question
    adapted = install_adapted(cmd_mox)
    adapted.clock.wraps(answer(read_number))
    run = run_gate(
        cmd_mox,
        tmp_path,
        evidence_state=FRESH,
        extra_environment=adapted.variables(cmd_mox),
    )

    assert run.result.returncode == 0, run.result.stderr
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert len(adapted.clock.calls) == read_number + 1, (
        "the failing operation must still publish its closing reading, and the "
        f"chain must stop there; read {read_number} of {adapted.clock.calls!r}"
    )
    assert operation_records(run.metrics, operation)[-1]["labels"] == (
        expected_operation_labels(
            CANARY_BY_OPERATION[operation], operation, "failure", "unknown"
        )
    ), "an unusable reading must refuse the operation that asked for it"
    assert run.outputs["gate-outcome"] == "failure", (
        "an unusable reading must reach the workflow output boundary"
    )
    assert run.outputs["gate-error-category"] == "unknown", (
        "an unusable reading must keep the bounded unknown category"
    )
    assert_failure_trace_sequence(run.traces, operation, "unknown")


def test_a_refused_closing_reading_still_refuses_the_gate(
    cmd_mox: CmdMox,
    tmp_path: Path,
) -> None:
    """Fail the final clock read of a run that would otherwise admit.

    Every earlier reading succeeds, so the only fault is the monotonic read
    that closes ``verify_evidence``. The operation's own result is replaced by
    ``failure``/``unknown`` with the defined zero duration, which is what
    separates a refused reading from a reading that merely costs its duration.
    """
    adapted = install_adapted(cmd_mox)
    adapted.clock.wraps(fail_nth_call(FINAL_CLOCK_READ, CLOCK_DIAGNOSTIC))
    run = run_gate(
        cmd_mox,
        tmp_path,
        evidence_state=FRESH,
        extra_environment=adapted.variables(cmd_mox),
    )

    assert run.result.returncode == 0, run.result.stderr
    METRICS_VALIDATOR.validate_metrics(run.metrics)
    METRICS_VALIDATOR.validate_traces(run.traces)
    assert len(adapted.clock.calls) == FINAL_CLOCK_READ, (
        "the case must spoil the run's last clock invocation and no later one"
    )
    assert operation_records(run.metrics, "verify_evidence")[-1]["labels"] == (
        expected_operation_labels(
            CANARY_BY_OPERATION["verify_evidence"],
            "verify_evidence",
            "failure",
            "unknown",
        )
    ), "a refused closing reading must refuse the operation it bounds"
    assert operation_duration(run.metrics, "verify_evidence") == 0, (
        "a refused closing reading must report the defined zero-duration fallback"
    )
    assert run.outputs["gate-outcome"] == "failure", (
        "a refused closing reading must reach the workflow output boundary"
    )
    assert run.outputs["gate-error-category"] == "unknown", (
        "a refused closing reading must keep the bounded unknown category"
    )
