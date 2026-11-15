"""Orchestrate the RFC 0005 release-admission gate.

This module is the composition root's behaviour half. It runs the five
admission operations in their fixed order, classifies each from the policy
module, and writes the three artefacts through the telemetry module. Nothing
here decides policy and nothing here runs a command directly, so both layers
stay separately testable. What the gate was *told* -- the environment variables,
their defaults, and the artefact paths -- is :mod:`configuration`; this module
asks what it then *does*.

Two behaviours are reproduced rather than improved, because the runtime tests
freeze them:

- Evidence is verified, not asserted. ``check_scan_freshness`` reads the
  manifest and refuses a timestamp outside the window; ``verify_evidence`` then
  requires that manifest to name this repository, revision, tag and run, to
  report both required canaries as successful, and to still be inside its
  window at that later moment. A freshness check passing is therefore never
  sufficient for admission.
- Bash's trailing-newline handling. A command substitution strips trailing
  newlines from a revision, but the value embedded in the next command's
  argument vector keeps them; the comparison between the two is what turns a
  revision ending in a newline into a bounded mismatch.

Evidence arrives through the :class:`~.filesystem.FileSystem` port rather than
being opened here, so the byte bound and the decoding are observable without a
real file, and the wall clock arrives through the same adapter the durations
already use.

Bounding is native. The shell wrapped each command in GNU ``timeout``, which
reported ``124`` when the command accepted ``SIGTERM`` and ``137`` when it had
to be killed. :mod:`bounding` reproduces the grace and the escalation, but it
reports an elapsed bound as ``None`` rather than as the status of whichever
signal ended the command, so every elapsed bound reaches this module's outcome
vocabulary as a timeout and the killed variant is not one a bound produces.
"""

import dataclasses
import math
import typing as typ

from . import admission_evidence, commands, evidence, policy, records, telemetry

if typ.TYPE_CHECKING:
    from . import filesystem
    from .configuration import Configuration

#: The five operations, in order, with the canary each belongs to.
OPERATIONS: typ.Final[tuple[tuple[policy.Canary, policy.Operation], ...]] = (
    (policy.Canary.NONE, policy.Operation.RESOLVE_TAG_COMMIT),
    (policy.Canary.RELEASE_CANDIDATE, policy.Operation.FETCH_CANDIDATE_REVISION),
    (policy.Canary.HISTORY_SCAN, policy.Operation.FETCH_WORKFLOW_RUN),
    (policy.Canary.HISTORY_SCAN, policy.Operation.CHECK_SCAN_FRESHNESS),
    (policy.Canary.HISTORY_SCAN, policy.Operation.VERIFY_EVIDENCE),
)


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


class Gate:
    """Run the admission once, accumulating the record the shell produced."""

    def __init__(
        self, configuration: Configuration, file_system: filesystem.FileSystem
    ) -> None:
        """Bind the resolved configuration, the filesystem port, and the record.

        The port is a parameter rather than a lookup so that the evidence read
        can be observed, and so a test can hand in a manifest without arranging
        a directory tree. It reaches the evidence decisions through
        :class:`~.admission_evidence.AdmissionEvidence`, which also takes this
        gate's own wall-clock reader so the window is measured where the record
        is written.
        """
        self._configuration = configuration
        self._evidence = admission_evidence.AdmissionEvidence(
            configuration, file_system, self._read_wall_clock
        )
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
        The maximum evidence age is checked the same way, so a mistyped bound
        cannot silently widen the window.

        Returns
        -------
        bool
            Whether the configuration was refused, in which case no operation
            may run.
        """
        configuration = self._configuration
        checks = (
            policy.is_operation_timeout(configuration.timeout_seconds),
            policy.is_admission_enforcement(configuration.enforcement),
            evidence.is_evidence_max_age(configuration.max_evidence_age_seconds),
        )
        if all(checks):
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
            output=configuration.output,
            record=records.GateRecord(
                outcome=self._outcome,
                error_category=self._error_category,
                enforcement=policy.enforcement_label(configuration.enforcement),
                candidate_commit=policy.publishable_revision(configuration.revision),
            ),
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
        """Read the monotonic clock once, reporting an unsuccessful exit."""
        result = commands.read_monotonic_seconds(self._configuration.programs)
        return result.output, result.outcome != policy.CommandOutcome.SUCCEEDED

    def _read_wall_clock(self) -> tuple[float, bool]:
        """Read the wall clock once, reporting an exit or reading it cannot use.

        A reading that is not a finite number is refused rather than compared:
        ``nan`` makes every comparison false, so accepting it would turn a
        broken adapter into a manifest that is always fresh.

        Returns
        -------
        tuple
            The seconds since the epoch, and whether the reading is unusable.
        """
        result = commands.read_wall_clock_seconds(self._configuration.programs)
        if result.outcome != policy.CommandOutcome.SUCCEEDED:
            return 0.0, True
        try:
            reading = float(result.output)
        except ValueError:
            return 0.0, True
        return (reading, False) if math.isfinite(reading) else (0.0, True)

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
                self._apply(self._evidence.check_freshness())
            case _:
                self._apply(self._evidence.admit())

    def _resolve_tag_commit(self) -> None:
        """Resolve the candidate through the API and compare it with ``GITHUB_SHA``.

        A publishing run names the tag it is about to release, so the API is
        asked what that tag resolves to and the answer is compared with the
        candidate: a tag that moved between the workflow's read and this one is
        a mismatch. The tag is asked about as ``tags/<name>`` rather than by
        its bare name, because the commits endpoint resolves a bare name
        against branches as well, so a branch sharing the release tag's name
        would answer for a tag that does not exist. A rehearsal names no tag,
        so the revision is resolved and compared with itself, which keeps the
        request contract and the self-check without inventing a tag.

        The comparison is a policy decision about two revisions, so it runs only
        once the command succeeded; anything else is classified by its own
        outcome and the operation's ``api_error`` category.
        """
        tag = self._configuration.tag
        subject = f"tags/{tag}" if tag else self._configuration.revision
        result = commands.github_resolve_commit(
            self._configuration.programs,
            repository=self._configuration.repository,
            revision=subject,
            timeout_seconds=int(self._configuration.timeout_seconds),
        )
        self._apply(
            policy.classify_commit_resolution(
                self._configuration.revision, result.output
            )
            if result.outcome == policy.CommandOutcome.SUCCEEDED
            else policy.classify_command_result(result, policy.ErrorCategory.API_ERROR)
        )

    def _fetch_candidate_revision(self) -> None:
        """Fetch the candidate revision from ``origin``.

        The fetch is reported rather than parsed, so the only decision is
        whether it succeeded. Any other outcome enters
        :func:`policy.classify_command_result` with ``fetch_error`` as the
        caller's own category, so only a timeout is re-classified.
        """
        result = commands.git_fetch_revision(
            self._configuration.programs,
            revision=self._configuration.revision,
            timeout_seconds=int(self._configuration.timeout_seconds),
        )
        self._apply(
            policy.classify_command_result(result, policy.ErrorCategory.FETCH_ERROR)
        )

    def _fetch_workflow_run(self) -> None:
        """Ask the API which commit the run named by ``GITHUB_RUN_ID`` started on.

        The answer must be the candidate. An empty answer is the shell's failed
        command substitution -- an ``api_error`` rather than a revision to
        compare -- and a different revision means this run is not the release's
        run, which is a candidate mismatch.
        """
        result = commands.github_current_workflow_run(
            self._configuration.programs,
            repository=self._configuration.repository,
            run_id=self._configuration.run_id,
            timeout_seconds=int(self._configuration.timeout_seconds),
        )
        if result.outcome != policy.CommandOutcome.SUCCEEDED:
            self._apply(
                policy.classify_command_result(result, policy.ErrorCategory.API_ERROR)
            )
            return
        if not result.output:
            self._refuse(policy.ErrorCategory.API_ERROR)
            return
        self._apply(
            policy.classify_run_head(result.output, self._configuration.revision)
        )

    def _refuse(self, error_category: policy.ErrorCategory) -> None:
        """Record the operation in progress as failed, for a bounded category."""
        self._apply(policy.Classification(policy.Outcome.FAILURE, error_category))

    def _apply(self, result: policy.Classification) -> None:
        """Adopt a policy classification for the operation in progress."""
        self._result.outcome = result.outcome
        self._result.error_category = result.error_category
