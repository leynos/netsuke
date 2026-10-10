"""Classify RFC 0005 release-admission outcomes without side effects.

Every decision the gate makes about *what happened* lives here, expressed as a
pure function from observed values to an outcome and error category. None of
it runs a command, reads a file, or writes a record, which is what makes the
admission policy reviewable on its own and testable without a filesystem.

The module also owns the bounded vocabularies. They are the gate's operator
contract -- the workflow's summary step and the published artefacts both
key off them -- so they are enumerated once, here, and every other module
imports them rather than repeating a literal.
"""

import enum
import re
import typing as typ

#: Operation timeout accepted by the gate, in seconds.
DEFAULT_OPERATION_TIMEOUT_SECONDS = 30
MAX_OPERATION_TIMEOUT_SECONDS = 300
#: Longest run of digits a timeout may contain before it is rejected outright.
MAX_OPERATION_TIMEOUT_DIGITS = 3

#: The one numeric shape a metric value or trace duration may take: unsigned
#: digits with an optional fractional part. Anything else -- an exponent, a
#: sign, a bare decimal point -- was dropped by the shell's own test, and the
#: records that carried it were never written.
_METRIC_VALUE = re.compile(r"[0-9]+([.][0-9]+)?")

#: Enforcement mode values. Observation is the default when unset.
ADMISSION_OBSERVATION_MODE = "false"
ADMISSION_ENFORCEMENT_MODE = "true"

#: Evidence state meaning "a scan ran for this revision".
EVIDENCE_FRESH = "fresh"

GATE_METRIC = "netsuke_release_admission_gate_total"
OPERATION_METRIC = "netsuke_release_admission_operation_total"
DURATION_METRIC = "netsuke_release_admission_operation_duration_seconds"

TRACE_OPERATION = "operation_complete"
TRACE_GATE = "gate_complete"
TRACE_WORKFLOW_OUTPUT = "workflow_output_delivery"
TRACE_DELIVERY = "trace_delivery"

DEFAULT_EVIDENCE_STATE = "missing"


class Vocabulary(enum.StrEnum):
    """Give each fixed vocabulary the one membership question the gate asks.

    The gate validates every value before it records it, and the value is
    always checked against one of these enumerations. Asking that question via
    a provided class method rather than through a per-vocabulary predicate
    keeps the answer in one place: a new vocabulary inherits it, and cannot
    bring a copy of its own.

    The one membership test that is *not* an enumeration member,
    :func:`is_admission_enforcement`, stays a function on purpose. A
    two-member set is not worth an enumeration, and that helper answers about
    the admission mode rather than about a record's vocabulary.
    """

    @classmethod
    def contains(cls, value: str) -> bool:
        """Return whether *value* is a member of this vocabulary.

        Returns
        -------
        bool
            Whether the value names a member of this enumeration.

        Examples
        --------
        >>> Outcome.contains("success"), Outcome.contains("")
        (True, False)
        """
        return value in cls


class Canary(Vocabulary):
    """Name the canary an operation belongs to, for metric labelling."""

    HISTORY_SCAN = "history_scan"
    RELEASE_CANDIDATE = "release_candidate"
    NONE = "none"


class Operation(Vocabulary):
    """Name one of the five fixed admission operations."""

    RESOLVE_TAG_COMMIT = "resolve_tag_commit"
    FETCH_CANDIDATE_REVISION = "fetch_candidate_revision"
    FETCH_WORKFLOW_RUN = "fetch_workflow_run"
    CHECK_SCAN_FRESHNESS = "check_scan_freshness"
    VERIFY_EVIDENCE = "verify_evidence"


class Outcome(Vocabulary):
    """Record whether an operation or the gate succeeded."""

    SUCCESS = "success"
    FAILURE = "failure"
    UNKNOWN = "unknown"


class ErrorCategory(Vocabulary):
    """Record why an operation or the gate did not succeed.

    The categories are deliberately coarse: they say which decision failed, so
    an operator reads the same set of words on every run and never sees a
    message body, a path, or an identifier in the published artefacts.
    """

    NONE = "none"
    API_ERROR = "api_error"
    FETCH_ERROR = "fetch_error"
    STALE_EVIDENCE = "stale_evidence"
    MISSING_EVIDENCE = "missing_evidence"
    MISMATCH = "mismatch"
    TIMEOUT = "timeout"
    UNKNOWN = "unknown"


class TraceEvent(Vocabulary):
    """Name the four bounded trace boundaries the gate emits."""

    OPERATION_COMPLETE = "operation_complete"
    GATE_COMPLETE = "gate_complete"
    WORKFLOW_OUTPUT_DELIVERY = "workflow_output_delivery"
    TRACE_DELIVERY = "trace_delivery"


class CommandStatus(enum.IntEnum):
    """Interpret the exit status of a bounded command.

    ``timeout`` reports ``124`` when the command accepted ``SIGTERM`` and
    ``137`` when it had to be killed, so both mean the operation ran out of
    time. Every other status -- including the ``126`` and ``127`` a missing or
    non-executable program produces -- is handed to the caller's own category,
    because a broken adapter is not a timed-out one.
    """

    TIMEOUT_BY_TERM = 124
    TIMEOUT_BY_KILL = 137


#: Statuses that mean a bounded command ran out of time rather than failed.
TIMEOUT_STATUSES: frozenset[int] = frozenset(int(status) for status in CommandStatus)


class Classification(typ.NamedTuple):
    """Hold the outcome and error category one policy decision produced."""

    outcome: Outcome
    error_category: ErrorCategory


def classify_command_failure(
    returncode: int, error_category: ErrorCategory
) -> Classification:
    """Classify a bounded command that exited unsuccessfully.

    A status of ``124`` or ``137`` means the command was killed for exceeding
    its timeout, whatever category the caller would otherwise use; anything
    else keeps the caller's category.

    Returns
    -------
    Classification
        The failure outcome and its category.

    Examples
    --------
    >>> classify_command_failure(124, ErrorCategory.API_ERROR).error_category
    <ErrorCategory.TIMEOUT: 'timeout'>
    >>> classify_command_failure(1, ErrorCategory.API_ERROR).error_category
    <ErrorCategory.API_ERROR: 'api_error'>
    """
    if returncode in TIMEOUT_STATUSES:
        return Classification(Outcome.FAILURE, ErrorCategory.TIMEOUT)
    return Classification(Outcome.FAILURE, error_category)


def classify_commit_resolution(revision: str, resolved_revision: str) -> Classification:
    """Classify the commit a tag resolution returned against the candidate.

    The comparison is between the revision the workflow supplied and the one
    the API reported. They differ when the tag moved between the workflow's
    read and this gate's -- a mismatch the admission must not accept.

    Returns
    -------
    Classification
        Success when the two agree, otherwise a mismatch.

    Examples
    --------
    >>> classify_commit_resolution("abc", "abc").outcome
    <Outcome.SUCCESS: 'success'>
    >>> classify_commit_resolution("abc", "def").error_category
    <ErrorCategory.MISMATCH: 'mismatch'>
    """
    if revision == resolved_revision:
        return Classification(Outcome.SUCCESS, ErrorCategory.NONE)
    return Classification(Outcome.FAILURE, ErrorCategory.MISMATCH)


def classify_scan_freshness(evidence_state: str) -> Classification:
    """Classify whether a recorded scan covers the candidate revision.

    The gate has no producer of its own yet, so the state arrives from the
    environment. ``fresh`` is the only state that admits the revision; a stale
    scan and a missing one are different operator problems, so they keep
    different categories; anything else is reported as unknown rather than
    forced into one of the two.

    Returns
    -------
    Classification
        The freshness decision for the supplied state.

    Examples
    --------
    >>> classify_scan_freshness("fresh").outcome
    <Outcome.SUCCESS: 'success'>
    >>> classify_scan_freshness("stale").error_category
    <ErrorCategory.STALE_EVIDENCE: 'stale_evidence'>
    """
    if evidence_state == EVIDENCE_FRESH:
        return Classification(Outcome.SUCCESS, ErrorCategory.NONE)
    if evidence_state == "stale":
        return Classification(Outcome.FAILURE, ErrorCategory.STALE_EVIDENCE)
    if evidence_state in {"missing", ""}:
        return Classification(Outcome.FAILURE, ErrorCategory.MISSING_EVIDENCE)
    return Classification(Outcome.FAILURE, ErrorCategory.UNKNOWN)


def classify_evidence(evidence_state: str, workflow_run_id: str) -> Classification:
    """Classify whether the evidence the revision needs was produced.

    This is the intentional contradiction with :func:`classify_scan_freshness`,
    and it is deliberately strict: a *fresh* state always fails here, and a
    state that is not fresh fails whenever it names no workflow run. Success is
    therefore reserved for the one shape the freshness check already refused --
    a non-fresh state that still names the run that produced it. The gate
    refuses a fresh state outright because freshness is the assertion it is
    checking, not the evidence it needs, and the alternative is admitting a
    revision whose evidence cannot be traced.

    Returns
    -------
    Classification
        Failure for a fresh state, or for any state without a run identifier;
        success otherwise.

    Examples
    --------
    >>> classify_evidence("stale", "1001").outcome
    <Outcome.SUCCESS: 'success'>
    >>> classify_evidence("fresh", "1001").error_category
    <ErrorCategory.MISSING_EVIDENCE: 'missing_evidence'>
    """
    if evidence_state == EVIDENCE_FRESH or not workflow_run_id:
        return Classification(Outcome.FAILURE, ErrorCategory.MISSING_EVIDENCE)
    return Classification(Outcome.SUCCESS, ErrorCategory.NONE)


def is_operation_timeout(value: str) -> bool:
    """Return whether a value is an acceptable operation timeout.

    A timeout must be a decimal integer with no sign and at most three digits,
    in the range ``1``..``300``. A leading zero is *rejected*: the shell's
    pattern was ``^[1-9][0-9]*$``, so ``030`` never reached the numeric
    comparison and the gate refused it before running any operation.

    Returns
    -------
    bool
        Whether the value is an acceptable bounded timeout.

    Examples
    --------
    >>> is_operation_timeout("30"), is_operation_timeout("030")
    (True, False)
    """
    return (
        value.isascii()
        and value.isdigit()
        and value[0] != "0"
        and len(value) <= MAX_OPERATION_TIMEOUT_DIGITS
        and int(value) <= MAX_OPERATION_TIMEOUT_SECONDS
    )


def is_admission_enforcement(value: str) -> bool:
    """Return whether a value is one of the two admission modes.

    Returns
    -------
    bool
        Whether the value names the observation or the enforcement mode.

    Examples
    --------
    >>> is_admission_enforcement("false"), is_admission_enforcement("")
    (True, False)
    """
    return value in {ADMISSION_OBSERVATION_MODE, ADMISSION_ENFORCEMENT_MODE}


def is_metric_value(value: str) -> bool:
    """Return whether a rendered value is an acceptable metric number.

    The gate validates the *text* of every value it records, including
    durations it computed itself, so a float Python renders in exponent form
    fails this check and the record that carried it is dropped rather than
    silently rewritten.

    Returns
    -------
    bool
        Whether the text is a JSON number of the shape the shell accepted.

    Examples
    --------
    >>> is_metric_value("1"), is_metric_value("1e-05")
    (True, False)
    """
    return _METRIC_VALUE.fullmatch(value) is not None
