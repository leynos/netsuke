"""Orchestrate the RFC 0005 release-admission gate and its configuration.

This module is the composition root. It reads the gate's configuration from
the environment, runs the five admission operations in their fixed order,
classifies each from the policy module, and writes the three artefacts through
the telemetry module. Nothing here decides policy and nothing here runs a
command directly, so both layers stay separately testable.

Two behaviours are reproduced rather than improved, because the runtime tests
freeze them:

- The bash ``fresh`` contradiction. ``check_scan_freshness`` admits a fresh
  state, but ``verify_evidence`` refuses it unless the state is *not* fresh and
  a workflow run identifier is present. A freshness check passing is therefore
  never sufficient for admission.
- Bash's trailing-newline handling. A command substitution strips trailing
  newlines from a revision, but the value embedded in the next command's
  argument vector keeps them; the comparison between the two is what turns a
  revision ending in a newline into a bounded mismatch.

Bounding is native. The shell wrapped each command in GNU ``timeout`` and
reported ``124`` (or ``137``) when it elapsed; :func:`commands.run_bounded`
reproduces both the grace and the classification.
"""

import dataclasses
import os
import typing as typ
from pathlib import Path

from . import commands, policy, records, telemetry

#: Environment variables the gate requires, and the diagnostic each produces.
REPOSITORY_VARIABLE = "GITHUB_REPOSITORY"
REPOSITORY_DIAGNOSTIC = "GITHUB_REPOSITORY must identify the release repository"
REVISION_VARIABLE = "GITHUB_SHA"
REVISION_DIAGNOSTIC = "GITHUB_SHA must identify the release candidate revision"
OUTPUT_VARIABLE = "GITHUB_OUTPUT"
OUTPUT_DIAGNOSTIC = "GITHUB_OUTPUT must identify the workflow output file"

#: Environment variables the gate reads with a documented default.
TIMEOUT_VARIABLE = "NETSUKE_RELEASE_ADMISSION_OPERATION_TIMEOUT_SECONDS"
ENFORCE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_ENFORCE"
EVIDENCE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_EVIDENCE_STATE"
METRICS_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_METRICS_FILE"
TRACE_FILE_VARIABLE = "NETSUKE_RELEASE_ADMISSION_TRACE_FILE"

#: The default artefact paths, relative to ``RUNNER_TEMP`` or the fallback.
METRICS_FILE_NAME = "netsuke-release-admission-metrics.jsonl"
TRACE_FILE_NAME = "netsuke-release-admission-traces.jsonl"
# The shell used ``${RUNNER_TEMP:-/tmp}``, so the fallback directory is part of
# the gate's contract rather than a temporary file this module invents.
DEFAULT_TEMPORARY_DIRECTORY = "/tmp"  # ruff: ignore[hardcoded-temp-file] - the documented fallback, never a temporary file.

#: The variable that selected a file on standard error before this port did.
DIAGNOSTIC_PREFIX = "release-admission"

#: The five operations, in order, with the canary each belongs to.
OPERATIONS: typ.Final[tuple[tuple[policy.Canary, policy.Operation], ...]] = (
    (policy.Canary.NONE, policy.Operation.RESOLVE_TAG_COMMIT),
    (policy.Canary.RELEASE_CANDIDATE, policy.Operation.FETCH_CANDIDATE_REVISION),
    (policy.Canary.HISTORY_SCAN, policy.Operation.FETCH_WORKFLOW_RUN),
    (policy.Canary.HISTORY_SCAN, policy.Operation.CHECK_SCAN_FRESHNESS),
    (policy.Canary.HISTORY_SCAN, policy.Operation.VERIFY_EVIDENCE),
)


@dataclasses.dataclass(frozen=True, slots=True)
class Configuration:
    """Hold the gate's resolved environment, with the shell's own defaults."""

    repository: str
    revision: str
    output: str
    evidence_state: str
    timeout_seconds: str
    enforcement: str
    metrics_file: Path
    trace_file: Path
    programs: commands.Programs
    sinks: telemetry.Sinks


class ConfigurationError(Exception):
    """Report a required variable that is unset or empty.

    The message is the diagnostic itself, so the caller prints exactly the
    sentence the shell's ``:?`` form produced and nothing derived from the
    environment.
    """


def load_configuration() -> Configuration:
    """Read the gate's configuration, creating the two artefact files.

    Returns
    -------
    Configuration
        The resolved environment.

    Notes
    -----
    A required variable that is unset or empty is refused by :func:`_required`,
    whose :class:`ConfigurationError` carries the diagnostic itself so the
    caller prints exactly the sentence the shell's ``:?`` form produced.
    """
    repository = _required(REPOSITORY_VARIABLE, REPOSITORY_DIAGNOSTIC)
    revision = _required(REVISION_VARIABLE, REVISION_DIAGNOSTIC)
    temporary = Path(os.environ.get("RUNNER_TEMP") or DEFAULT_TEMPORARY_DIRECTORY)
    metrics_file = Path(
        os.environ.get(METRICS_FILE_VARIABLE) or temporary / METRICS_FILE_NAME
    )
    trace_file = Path(
        os.environ.get(TRACE_FILE_VARIABLE) or temporary / TRACE_FILE_NAME
    )
    _create_artefacts(metrics_file, trace_file)
    output = _required(OUTPUT_VARIABLE, OUTPUT_DIAGNOSTIC)
    programs = commands.Programs.from_environment()
    return Configuration(
        repository=repository,
        revision=revision,
        output=output,
        evidence_state=(
            os.environ.get(EVIDENCE_VARIABLE) or policy.DEFAULT_EVIDENCE_STATE
        ),
        timeout_seconds=(
            os.environ.get(TIMEOUT_VARIABLE)
            or str(policy.DEFAULT_OPERATION_TIMEOUT_SECONDS)
        ),
        # The shell wrote ``${NETSUKE_RELEASE_ADMISSION_ENFORCE-$default}``,
        # whose dash substitutes only when the variable is *unset*: an exported
        # empty value was kept and refused by the gate's own mode check rather
        # than replaced here. The membership test below is therefore
        # load-bearing. ``os.environ.get(ENFORCE_VARIABLE, default)`` would
        # substitute an empty value and let a refused configuration run as
        # observation mode, so the apparent simplification is not one.
        enforcement=(
            os.environ[ENFORCE_VARIABLE]  # ruff: ignore[if-else-block-instead-of-dict-get] - see above; a dict-get default is not equivalent to Bash's ``-``.
            if ENFORCE_VARIABLE in os.environ
            else policy.ADMISSION_OBSERVATION_MODE
        ),
        metrics_file=metrics_file,
        trace_file=trace_file,
        programs=programs,
        sinks=telemetry.Sinks(
            metrics_file=metrics_file,
            trace_file=trace_file,
            metrics_sink=os.environ.get(commands.METRICS_SINK_VARIABLE) or "",
            output_sink=os.environ.get(commands.OUTPUT_SINK_VARIABLE) or "",
            trace_sink=os.environ.get(commands.TRACE_SINK_VARIABLE) or "",
        ),
    )


def _required(variable: str, diagnostic: str) -> str:
    """Return a required variable, refusing an unset or empty value."""
    value = os.environ.get(variable)
    if not value:
        raise ConfigurationError(diagnostic)
    return value


def _create_artefacts(*paths: Path) -> None:
    """Create the artefact files empty, as the shell truncated them."""
    for path in paths:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"")


@dataclasses.dataclass(slots=True)
class OperationResult:
    """Hold one operation's classification and its rendered duration.

    ``duration`` is the *text* the shell would have carried: digits with an
    optional fraction, or the empty string when the duration could not be
    computed. It is kept as text because every consumer -- the metric record,
    the trace record, and the vocabulary check -- validated text, and a
    duration that Python would render in exponent form must fail that check
    rather than be silently rewritten.
    """

    outcome: policy.Outcome = policy.Outcome.UNKNOWN
    error_category: policy.ErrorCategory = policy.ErrorCategory.UNKNOWN
    duration: str = "0"
    workflow_run_id: str = ""


class Gate:
    """Run the admission once, accumulating the record the shell produced."""

    def __init__(self, configuration: Configuration) -> None:
        """Bind the resolved configuration and the gate's mutable record."""
        self._configuration = configuration
        self._outcome = policy.Outcome.UNKNOWN
        self._error_category = policy.ErrorCategory.UNKNOWN
        self._trace_sink_failed = False
        self._configuration_refused = False
        self._result = OperationResult()

    def should_stop(self) -> bool:
        """Refuse an unusable configuration, recording the gate as failed.

        The shell checked an unacceptable operation timeout and an unacceptable
        admission mode before running anything, having already created the
        artefact files and armed the exit trap. Both checks set the gate to
        ``failure``/``unknown`` and exited ``1`` -- the mode's own value never
        decided the status, so a refused *observation* mode still failed the
        step. :meth:`finish` reads the recorded refusal rather than the mode.

        Returns
        -------
        bool
            Whether the configuration was refused, in which case no operation
            may run.
        """
        configuration = self._configuration
        if policy.is_operation_timeout(
            configuration.timeout_seconds
        ) and policy.is_admission_enforcement(configuration.enforcement):
            return False
        self._outcome = policy.Outcome.FAILURE
        self._error_category = policy.ErrorCategory.UNKNOWN
        self._configuration_refused = True
        return True

    def run_operations(self) -> None:
        """Run the five operations in order, stopping at the first failure."""
        for canary, operation in OPERATIONS:
            if not self._run_operation(canary, operation):
                self._outcome = policy.Outcome.FAILURE
                self._error_category = self._result.error_category
                return
        self._outcome = policy.Outcome.SUCCESS
        self._error_category = policy.ErrorCategory.NONE

    def finish(self) -> int:
        """Write the gate record and return the status the gate must exit with.

        Returns
        -------
        int
            ``0`` for a gate that ran in observation mode, ``1`` for one that
            enforced a failure, and otherwise the failing sink's own status,
            which ``set -e`` adopted as the gate's.
        """
        configuration = self._configuration
        status = telemetry.record_gate_result(
            configuration.sinks,
            output=Path(configuration.output),
            record=records.GateRecord(self._outcome, self._error_category),
            trace_sink_failed=self._trace_sink_failed,
        )
        if status:
            return status
        if self._outcome == policy.Outcome.SUCCESS:
            return 0
        if configuration.enforcement == policy.ADMISSION_ENFORCEMENT_MODE:
            return 1
        return 1 if self._configuration_refused else 0

    def _run_operation(
        self, canary: policy.Canary, operation: policy.Operation
    ) -> bool:
        """Run one operation under a clock reading front and back.

        The clock is read twice around every operation, including the two that
        run no command at all. A clock read that exits unsuccessfully forces
        the operation to ``failure``/``unknown`` with a duration of ``0``; a
        read that succeeds but prints something unparsable leaves the
        operation's own result alone and costs only the duration.

        Returns
        -------
        bool
            Whether the operation succeeded, which decides how the gate ends.
        """
        started, clock_failed = self._read_clock()
        self._dispatch(operation)
        finished, failed = self._read_clock()
        clock_failed = clock_failed or failed
        if clock_failed:
            self._result.outcome = policy.Outcome.FAILURE
            self._result.error_category = policy.ErrorCategory.UNKNOWN
            self._result.duration = "0"
        else:
            self._result.duration = commands.render_duration(started, finished)
        failed = telemetry.record_operation(
            self._configuration.sinks,
            records.OperationRecord(
                canary=canary,
                operation=operation,
                outcome=self._result.outcome,
                error_category=self._result.error_category,
                duration=self._result.duration,
            ),
        )
        self._trace_sink_failed = self._trace_sink_failed or failed
        return self._result.outcome == policy.Outcome.SUCCESS

    def _read_clock(self) -> tuple[str, bool]:
        """Read the clock adapter once, reporting an unsuccessful exit."""
        status, reading = commands.read_monotonic_seconds(self._configuration.programs)
        return reading, status != 0

    def _dispatch(self, operation: policy.Operation) -> None:
        """Run the named operation and adopt its policy classification.

        The two checks that run no command are the last two operations, and the
        final arm is what ``verify_evidence`` reaches: the enum has five
        members and the first three are named, so a value outside the
        vocabulary would still be classified as an evidence check. That is the
        shell's own ``case`` behaviour, whose ``*)`` arm was the else branch.
        """
        match operation:
            case policy.Operation.RESOLVE_TAG_COMMIT:
                self._resolve_tag_commit()
            case policy.Operation.FETCH_CANDIDATE_REVISION:
                self._fetch_candidate_revision()
            case policy.Operation.FETCH_WORKFLOW_RUN:
                self._fetch_workflow_run()
            case policy.Operation.CHECK_SCAN_FRESHNESS:
                self._apply(
                    policy.classify_scan_freshness(self._configuration.evidence_state)
                )
            case _:
                self._apply(
                    policy.classify_evidence(
                        self._configuration.evidence_state,
                        self._result.workflow_run_id,
                    )
                )

    def _resolve_tag_commit(self) -> None:
        """Resolve the candidate revision through the API and compare it."""
        status, resolved = commands.github_resolve_commit(
            self._configuration.programs,
            repository=self._configuration.repository,
            revision=self._configuration.revision,
            timeout_seconds=int(self._configuration.timeout_seconds),
        )
        self._apply(
            policy.classify_commit_resolution(self._configuration.revision, resolved)
            if status == 0
            else policy.classify_command_failure(status, policy.ErrorCategory.API_ERROR)
        )

    def _fetch_candidate_revision(self) -> None:
        """Fetch the candidate revision from ``origin``.

        A fetch that succeeds while the gate has already classified nothing is
        success/none, which is the shell's ``return 0`` from its fetch helper.
        Any other status enters :func:`policy.classify_command_failure` with
        ``fetch_error`` as the caller's own category, so only a timeout is
        re-classified.
        """
        status = commands.git_fetch_revision(
            self._configuration.programs,
            revision=self._configuration.revision,
            timeout_seconds=int(self._configuration.timeout_seconds),
        )
        self._apply(
            policy.Classification(policy.Outcome.SUCCESS, policy.ErrorCategory.NONE)
            if status == 0
            else policy.classify_command_failure(
                status, policy.ErrorCategory.FETCH_ERROR
            )
        )

    def _fetch_workflow_run(self) -> None:
        """Ask the API for the workflow run covering the candidate revision."""
        status, run_id = commands.github_find_workflow_run(
            self._configuration.programs,
            repository=self._configuration.repository,
            revision=self._configuration.revision,
            timeout_seconds=int(self._configuration.timeout_seconds),
        )
        self._result.workflow_run_id = run_id
        self._apply(
            policy.Classification(policy.Outcome.SUCCESS, policy.ErrorCategory.NONE)
            if status == 0
            else policy.classify_command_failure(status, policy.ErrorCategory.API_ERROR)
        )

    def _apply(self, result: policy.Classification) -> None:
        """Adopt a policy classification for the operation in progress."""
        self._result.outcome = result.outcome
        self._result.error_category = result.error_category
