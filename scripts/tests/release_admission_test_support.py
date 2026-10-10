r"""Record the frozen release-admission behaviour and its fixed assertions.

The Python gate replaces three Bash scripts, and its whole claim to be a
refactor rather than a rewrite is that no observer can tell the difference.
This docstring is that claim's evidence: it records what the Bash gate at
``origin/main`` (``48d4a596``) actually did, measured by running it, so the
Python entry point can be held to the same observations case for case.

This module is the support and export facade for the release-admission runtime
tests. It owns the frozen measurements below, the fixed label constructors and
failure classification, and it re-exports the subprocess harness from
:mod:`release_admission_test_harness` and the record assertions from
:mod:`release_admission_test_records`, so a runtime module imports one name.

Inputs
------
Every input is an environment variable. Absent, empty, and set are three
distinct states for several of them, and the defaults differ in kind:

- ``GITHUB_REPOSITORY`` and ``GITHUB_SHA`` are required. Bash's ``:?`` form
  aborts with a diagnostic when either is unset or empty.
- ``GITHUB_OUTPUT`` is required later, when the gate results are written.
- ``NETSUKE_RELEASE_ADMISSION_GH_ADAPTER`` (``gh``), ``..._GIT_ADAPTER``
  (``git``), and ``..._CLOCK_ADAPTER`` (``python3``) name the executables the
  gate runs. Each is resolved through ``PATH``.
- ``NETSUKE_RELEASE_ADMISSION_METRICS_SINK``, ``..._OUTPUT_SINK``, and
  ``..._TRACE_SINK`` default to the empty string, which selects an in-process
  append over an external adapter.
- ``NETSUKE_RELEASE_ADMISSION_METRICS_FILE`` and ``..._TRACE_FILE`` default to
  ``${RUNNER_TEMP:-/tmp}/netsuke-release-admission-metrics.jsonl`` and the
  matching ``...-traces.jsonl``.
- ``NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS`` defaults to ``30``
  and is valid only as a decimal integer in ``1``..``300``.
- ``NETSUKE_RELEASE_ADMISSION_ENFORCE`` defaults to ``false`` *only when
  unset*: Bash's ``${VAR-default}`` form keeps an empty value empty, and the
  empty string is not a valid enforcement mode.
- ``NETSUKE_RELEASE_ADMISSION_EVIDENCE_STATE`` defaults to ``missing``.

Outputs
-------
Three artefacts, all written before any external command runs:

- The metrics file, JSON Lines. Each record is ``name``, ``labels``,
  ``value`` in that key order. The operation counter's labels are ``canary``,
  ``operation``, ``outcome``, ``error_category``; the gate counter's are
  ``outcome``, ``error_category``; the duration metric's are ``operation``.
  Values are JSON numbers; Bash prints durations as they come from Python, so
  ``0`` is an integer and ``0.0`` is a float depending on the adapter.
- The trace file, JSON Lines, with keys ``event``, ``operation``, ``outcome``,
  ``error_category``, ``duration_seconds``.
- ``GITHUB_OUTPUT``, four ``key=value`` lines in the order ``gate-outcome``,
  ``gate-error-category``, ``metrics-file``, ``trace-file``.

Trace records are whole. A byte-level measurement of a fresh run reads eight
complete records in 1135 bytes, and a missing-evidence run seven in 989; the
long float `duration_seconds` values arrive intact. An earlier revision of this
docstring claimed a long duration arrived truncated, from a `$(...)` capture
that had stripped a trailing newline rather than from the file. The trace file
is appended to one record at a time and is never rewritten, so a partial final
line would be a genuine fault rather than an accepted shape.

Frozen behaviour
----------------
Measured on the shell gate with controlled adapters and the metrics, trace, and
output sinks configured. Those adapters were executable fakes reading
``NETSUKE_FAKE_*`` variables; :mod:`release_admission_test_scenarios` is their
port, deciding the same substitutions in the test process as cmd-mox wrappers,
so every measurement below still names the observation the Python gate must
reproduce:

- Missing evidence, observation mode: exit ``0``. Four operation counters and
  durations -- ``resolve_tag_commit``, ``fetch_candidate_revision``,
  ``fetch_workflow_run`` succeed, ``check_scan_freshness`` is
  ``failure``/``missing_evidence`` -- then the gate counter
  ``failure``/``missing_evidence``. ``verify_evidence`` never runs, because
  the freshness check stopped the chain. All four ``GITHUB_OUTPUT`` lines.
- Fresh evidence, observation mode: exit ``0``. All five operations run and
  ``verify_evidence`` is ``failure``/``missing_evidence`` even though the
  evidence is fresh. This is the intentional contradiction the gate encodes:
  a fresh state is only reachable with a workflow-run identifier, and the
  fixture supplies none. The gate counter repeats that category.
- Invalid ``OPERATION_TIMEOUT_SECONDS`` (``0``, ``301``, ``not-a-number``) or
  invalid ``ENFORCE`` (``""``, ``False``, ``observe``): exit ``1``, exactly one
  metric record -- the gate counter ``failure``/``unknown`` -- exactly three
  traces (``gate_complete``, ``workflow_output_delivery``, ``trace_delivery``),
  all four ``GITHUB_OUTPUT`` lines, and no external command at all.
- Missing ``GITHUB_OUTPUT``: exit ``1``, both JSON Lines files created and
  empty, no ``GITHUB_OUTPUT`` written, and a Bash diagnostic naming the line
  that tested it.
- Missing ``GITHUB_REPOSITORY`` or ``GITHUB_SHA``: exit ``1`` with neither
  JSON Lines file created, because the parameter expansion aborts before the
  files are opened.
- Metrics sink failing throughout: exit ``1``, metrics 0 lines, traces 5 lines
  (the five ``operation_complete`` records), outputs 0 lines. The first
  unguarded metric write is the gate's own, inside ``record_gate_result``, and
  it aborts the gate before ``gate_complete``.
- Output sink failing throughout: exit ``1``, metrics 11 lines (complete,
  including the gate counter), traces 6 lines (stops after ``gate_complete``),
  outputs 0 lines, because the workflow-output writes sit between
  ``gate_complete`` and the delivery traces.
- Trace sink failing throughout: exit ``0``, metrics 11 lines, traces 0 lines,
  outputs 4 lines, and eight ``release-admission trace sink failed``
  diagnostics -- one per attempted trace write. Trace delivery is fail-open by
  design; the gate's own result is untouched.
- A clock adapter that exits non-zero: the operation becomes
  ``failure``/``unknown`` with a zero duration, the chain stops there, the gate
  is ``failure``/``unknown``, and the workflow outputs carry that category. The
  same is true of either clock read, so a failure at the start and a failure at
  the finish are indistinguishable in the records.
- A clock adapter that prints a non-number: the count is unaffected -- the
  printed value only ever reaches a formatter -- and the artefact set depends
  only on *which* reads return garbage, not on whether the garbage is on a
  start read or a finish read. Garbage on any single read of an operation
  drops that operation's counter and duration together, and the float
  ``0.0`` reached every later operation and the gate, because the failing
  substitution leaves the shared variable holding the previous read's text.
  Garbage on every read of every operation writes five counters, no durations,
  and no ``operation_complete`` traces at all, with one Python ``ValueError``
  traceback and one ``metric labels are outside the fixed vocabulary`` pair per
  operation. Both shapes leave all later records intact, ``trace_delivery``
  ``failure``/``unknown``, and the exit status unchanged: the vocabulary
  rejection sets the same flag a sink failure does.
- Bash's required-input diagnostics name their own line, as
  ``<absolute script path>: line N: VAR: message``, for ``GITHUB_REPOSITORY``
  (line 56), ``GITHUB_SHA`` (line 57), and ``GITHUB_OUTPUT`` (line 195). The
  path and line number are a property of the Bash source and cannot be
  reproduced from Python; the Python gate keeps the message and drops the
  prefix. This is a stated wording difference, not an accident.
- Sink failures are only checked where the gate writes its own result. An
  operation's metric and trace writes sit inside a call the shell tests with
  ``!``, which suspends its abort-on-failure rule, so a sink that fails for
  every operation leaves the operation results and the operation count
  untouched -- the records are simply lost -- and only the gate's own record
  reports the failure. A sink that fails from the start therefore still logs
  every operation, and the exit status is decided by the gate record alone.
- An adapter the operating system refuses to launch -- a name that is not on
  ``PATH``, or a file without its execute bit -- is not a gate failure of its
  own kind. The shell reached it through GNU ``timeout``, which printed
  ``timeout: failed to run command '<program>': <reason>`` and exited ``127``
  for a missing program and ``126`` for any other refusal. Neither status is
  ``124`` or ``137``, so ``classify_command_failure`` keeps the operation's own
  category: a missing ``gh`` is ``api_error``, a missing ``git`` is
  ``fetch_error``. The Python gate prints its own sentence naming the program
  and reproducing the status, because cuprum raises ``FileNotFoundError`` or
  ``PermissionError`` where ``timeout`` returned a status. This is a stated
  wording difference, in the same position and count as the shell's.
- Every record is terminated with the newline ``printf '%s\n'`` added, in both
  the adapter path and the in-process append. The JSON Lines reader drops a
  final record that has none, so a byte-level comparison between the two gates
  shows the same lines only when both terminate the last one.
- ``GITHUB_OUTPUT`` is not the only target whose value can be unset: when a
  required variable aborts the gate before the artefact paths are computed, the
  shell's ``>>"$file"`` redirection carried no operand and Bash wrote the
  record to *its own standard output*. The Python port reproduces that for the
  metrics and trace appends rather than dropping the record, so a run that
  fails on ``GITHUB_REPOSITORY`` is byte-identical on stdout as well.
"""

from release_admission_test_doubles import (
    CLOCK_ARGUMENTS,
    GITHUB_REPOSITORY,
    REVISION,
    AdaptedBoundaries,
    Boundaries,
    GateRun,
    Subprocess,
    Wrapper,
    install_adapted,
    install_boundaries,
    reply_to_clock,
    reply_to_github,
    reply_to_sink,
    shim,
)
from release_admission_test_harness import (
    METRICS_VALIDATOR,
    PYTHON_PATH,
    SCRIPT_PATH,
    run_gate,
)
from release_admission_test_records import (
    assert_failure_trace_sequence,
    assert_identifiers_excluded_from_records,
    assert_identifiers_excluded_from_values,
    operation_duration,
    operation_records,
)

__all__ = (
    "CANARY_BY_OPERATION",
    "CLOCK_ARGUMENTS",
    "GITHUB_REPOSITORY",
    "METRICS_VALIDATOR",
    "PYTHON_PATH",
    "REVISION",
    "SCRIPT_PATH",
    "AdaptedBoundaries",
    "Boundaries",
    "GateRun",
    "Subprocess",
    "Wrapper",
    "assert_failure_trace_sequence",
    "assert_identifiers_excluded_from_records",
    "assert_identifiers_excluded_from_values",
    "expected_gate_labels",
    "expected_operation_labels",
    "install_adapted",
    "install_boundaries",
    "operation_duration",
    "operation_records",
    "reply_to_clock",
    "reply_to_github",
    "reply_to_sink",
    "run_gate",
    "shim",
)

CANARY_BY_OPERATION = {
    "resolve_tag_commit": "none",
    "fetch_candidate_revision": "release_candidate",
    "fetch_workflow_run": "history_scan",
    "check_scan_freshness": "history_scan",
    "verify_evidence": "history_scan",
}


def expected_operation_labels(
    canary: str, operation: str, outcome: str, error_category: str
) -> dict[str, str]:
    """Return fixed labels for one release-admission operation counter.

    Parameters
    ----------
    canary
        Bounded canary identifier assigned to the operation.
    operation
        Fixed operation name represented by the counter.
    outcome
        Bounded operation outcome.
    error_category
        Bounded error category, or ``"none"`` for success.

    Returns
    -------
    dict[str, str]
        Labels in the operation-counter schema order.

    Notes
    -----
    Contract invariants: return exactly ``canary``, ``operation``, ``outcome``,
    and ``error_category`` for validator vocabulary checks.
    """
    return {
        "canary": canary,
        "operation": operation,
        "outcome": outcome,
        "error_category": error_category,
    }


def expected_gate_labels(outcome: str, error_category: str) -> dict[str, str]:
    """Return fixed labels for the overall release-admission counter.

    Parameters
    ----------
    outcome
        Bounded overall gate outcome.
    error_category
        Bounded error category, or ``"none"`` for success.

    Returns
    -------
    dict[str, str]
        Labels in the gate-counter schema order.

    Notes
    -----
    Contract invariants: return exactly ``outcome`` and ``error_category`` for
    validator vocabulary checks.
    """
    return {"outcome": outcome, "error_category": error_category}
