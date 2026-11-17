"""Classify RFC 0005 release-admission outcomes without side effects.

Every decision the gate makes about *what happened* lives here, expressed as a
pure function from observed values to an outcome and error category. None of
it runs a command, reads a file, or writes a record, which is what makes the
admission policy reviewable on its own and testable without a filesystem.

The module also owns the bounded vocabularies. They are the gate's operator
contract -- the workflow's summary step and the published artefacts both
key off them -- so they are enumerated once, here, and every other module
imports them rather than repeating a literal.

What an external command did is one of those vocabularies, so
:class:`CommandOutcome` and the :class:`CommandResult` carrying it are declared
here, and :func:`classify_command_result` consumes them. The translation that
*produces* them -- a signal death, an elapsed bound, a launch the operating
system refused -- is not a decision and does not live here; it belongs to the
adapters that observed the operating system, and :mod:`statuses` performs it.
"""

import dataclasses
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

GATE_METRIC = "netsuke_release_admission_gate_total"
OPERATION_METRIC = "netsuke_release_admission_operation_total"
DURATION_METRIC = "netsuke_release_admission_operation_duration_seconds"


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


#: The canaries an evidence manifest must report before admission is granted.
#: #598 adds downstream canaries whose names need no change here, so this is
#: the closed set of *required* results, not the whole producer vocabulary:
#: presence is required here, success of every declared result.
REQUIRED_RESULTS: typ.Final[tuple[Canary, ...]] = (
    Canary.HISTORY_SCAN,
    Canary.RELEASE_CANDIDATE,
)


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

    ``malformed_evidence`` and ``candidate_mismatch`` split the two ways a
    supplied manifest can be unusable. The first says the document could not be
    read as the schema at all; the second says it read perfectly well and
    describes a different release. An operator fixes those in different places,
    so they are not one category.
    """

    NONE = "none"
    API_ERROR = "api_error"
    FETCH_ERROR = "fetch_error"
    STALE_EVIDENCE = "stale_evidence"
    MISSING_EVIDENCE = "missing_evidence"
    MALFORMED_EVIDENCE = "malformed_evidence"
    CANDIDATE_MISMATCH = "candidate_mismatch"
    MISMATCH = "mismatch"
    TIMEOUT = "timeout"
    UNKNOWN = "unknown"


class TraceEvent(Vocabulary):
    """Name the four bounded trace boundaries the gate emits."""

    OPERATION_COMPLETE = "operation_complete"
    GATE_COMPLETE = "gate_complete"
    WORKFLOW_OUTPUT_DELIVERY = "workflow_output_delivery"
    TRACE_DELIVERY = "trace_delivery"


class CommandOutcome(enum.StrEnum):
    """Name what an external command did, in the operating system's terms.

    The vocabulary lives here rather than in an adapter because a caller has to
    name the *decision* it reached, and the caller is the gate: an adapter
    reports that a command timed out, and the gate decides that a timed-out
    ``gh api`` is an API failure. Naming the decision beside the policy that
    consumes it is what keeps the adapters free to say only what they observed.

    ``LAUNCH_FAILED`` is separate from ``FAILED`` because the two tell an
    operator different things. A command that ran and failed may have failed
    for a reason its operation understands; a command the operating system
    refused to start did not run at all, so the fault is in the adapter
    configuration that names it.
    """

    SUCCEEDED = "succeeded"
    TIMED_OUT = "timed_out"
    FAILED = "failed"
    LAUNCH_FAILED = "launch_failed"


@dataclasses.dataclass(frozen=True, slots=True)
class CommandResult:
    """Carry what one external command produced, as the gate needs it.

    ``status`` is retained beside the outcome because the gate's own exit
    status is decided from it: a sink that died of ``SIGTERM`` must leave the
    step exiting ``143``, a number the outcome vocabulary alone cannot express.

    ``output`` is the command's retained standard output, empty for every
    outcome but success. A failed command substitution left Bash's variable
    empty, and the gate must not parse a diagnostic as though it were an
    answer.
    """

    outcome: CommandOutcome
    status: int
    output: str = ""


class Classification(typ.NamedTuple):
    """Hold the outcome and error category one policy decision produced."""

    outcome: Outcome
    error_category: ErrorCategory


def classify_command_result(
    result: CommandResult, error_category: ErrorCategory
) -> Classification:
    """Classify what an external command produced, given the caller's category.

    A command that ran out of time is ``timeout`` whatever the caller would
    otherwise say, because exceeding the bound is the more specific fault and
    the one an operator acts on. Every other unsuccessful command keeps the
    caller's category: a ``gh api`` that exited ``1`` is an API failure and a
    ``git fetch`` that exited ``1`` is a fetch failure, and neither the adapter
    nor this classifier has to know which is which. The category arrives from
    the call site that does know.

    Returns
    -------
    Classification
        Success for a command that succeeded, otherwise the failure outcome and
        its category.

    Examples
    --------
    >>> timed_out = CommandResult(CommandOutcome.TIMED_OUT, 124)
    >>> classify_command_result(timed_out, ErrorCategory.FETCH_ERROR).error_category
    <ErrorCategory.TIMEOUT: 'timeout'>
    >>> failed = CommandResult(CommandOutcome.FAILED, 1)
    >>> classify_command_result(failed, ErrorCategory.API_ERROR).error_category
    <ErrorCategory.API_ERROR: 'api_error'>
    """
    if result.outcome == CommandOutcome.SUCCEEDED:
        return Classification(Outcome.SUCCESS, ErrorCategory.NONE)
    if result.outcome == CommandOutcome.TIMED_OUT:
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


def classify_run_head(head_revision: str, candidate_revision: str) -> Classification:
    """Classify the commit a workflow-run lookup reported against the candidate.

    The gate asks the API for the run named by ``GITHUB_RUN_ID`` and requires
    that run's head to *be* the revision under release. Evidence produced by
    one run cannot admit another, so a difference here is a candidate mismatch
    rather than an API fault: the lookup succeeded and answered a different
    question.

    Returns
    -------
    Classification
        Success when the run's head is the candidate, otherwise a candidate
        mismatch.

    Examples
    --------
    >>> classify_run_head("abc", "abc").outcome
    <Outcome.SUCCESS: 'success'>
    >>> classify_run_head("def", "abc").error_category
    <ErrorCategory.CANDIDATE_MISMATCH: 'candidate_mismatch'>
    """
    if head_revision == candidate_revision:
        return Classification(Outcome.SUCCESS, ErrorCategory.NONE)
    return Classification(Outcome.FAILURE, ErrorCategory.CANDIDATE_MISMATCH)


#: The shape of a full Git object ID: lowercase hexadecimal, either the SHA-1
#: width the workflow's own metadata step validates or the SHA-256 width a
#: repository may be created with.
_FULL_REVISION = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")


def publishable_revision(revision: str) -> str:
    r"""Return *revision* when it is safe to publish, and ``""`` otherwise.

    The revision reaches ``GITHUB_OUTPUT``, a line-oriented file of
    ``key=value`` lines: a newline would forge a second line and invent an
    output. A value that is not a full object ID therefore publishes as empty,
    which the publication job's ``candidate-commit == github.sha`` comparison
    refuses. Only the *published* value is narrowed; the revision itself still
    reaches the commit query and the self-check unchanged.

    Returns
    -------
    str
        The revision when it is a full lowercase object ID, otherwise ``""``.

    Examples
    --------
    >>> publishable_revision("0123456789abcdef0123456789abcdef01234567")
    '0123456789abcdef0123456789abcdef01234567'
    >>> publishable_revision("0123456789abcdef0123456789abcdef01234567\n")
    ''
    """
    return revision if _FULL_REVISION.fullmatch(revision) else ""


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


def enforcement_label(value: str) -> str:
    """Return the bounded spelling of an admission mode for the workflow output.

    A refused mode is reported as observation, because that is what the run
    actually did: it wrote no failure exit. The publication job compares the
    output with ``'true'``, so a run that never enforced can never satisfy it.

    Returns
    -------
    str
        ``"true"`` only for the enforcement mode, otherwise ``"false"``.

    Examples
    --------
    >>> enforcement_label("true"), enforcement_label("observe")
    ('true', 'false')
    """
    return (
        ADMISSION_ENFORCEMENT_MODE
        if value == ADMISSION_ENFORCEMENT_MODE
        else ADMISSION_OBSERVATION_MODE
    )


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
