"""Read the evidence manifest and decide whether it admits the candidate.

This module owns the two evidence decisions and the read they both rest on.
``check_freshness`` reads the document through the supplied port, parses it, and
refuses any timestamp outside the window, returning an :class:`Observation` of
the document it read and the verdict it reached; ``admit`` takes that
observation and requires its document to name the candidate and report both
required results as successful, then re-reads the window before it answers. The
document a decision is made on is therefore an argument at the call site rather
than state this object carries from one call to the next. The second reading is
not redundant:
two clock commands and the telemetry writes separate the two decisions, so a
document observed a moment inside its window can be outside it by the time the
gate would admit on it. Admitting on the earlier reading would let the evidence
outlive the bound the gate publishes. The recheck runs only after the identity
and result checks passed, and can only replace that success with a refusal, so
it adds no way to admit; a refusal at the first check is carried through, so a
reading that moved back inside the window cannot revive evidence the gate has
already rejected.

The origin of the document is judged as well as its contents. Identity fields
are compared with the run, which shows the document agrees with the candidate;
it does not show who wrote it, because ``actions/download-artifact`` resolves
the current run and a reusable-workflow caller defines jobs in that run. An
enforcing run therefore admits only when the configuration reports that the
evidence can be attributed to a trusted producer, and refuses with
``untrusted_evidence`` otherwise. That flag is set by this repository's own
workflow, so the shipped value keeps publication blocked whatever a same-run
supplier uploads; binding the evidence to a named producer is #598's work.

Nothing here decides policy. Categories come from :mod:`policy`, the schema and
its classification from :mod:`evidence`, and both effects -- the byte read and
the wall-clock reading -- arrive as injected collaborators. The gate keeps the
orchestration: it decides the order of the five operations and writes the
record, so a change to what admission *means* is a change here and a change to
what the gate *does* is a change there.
"""

import collections.abc as cabc
import typing as typ

from . import evidence, policy

if typ.TYPE_CHECKING:
    from . import filesystem
    from .configuration import Configuration

#: A reading of the wall clock in seconds since the epoch, paired with whether
#: the reading is usable. The pair travels as one value because the number
#: alone cannot report an unusable reading: zero is a real instant.
type WallClock = cabc.Callable[[], tuple[float, bool]]


class Observation(typ.NamedTuple):
    """Hold the document the freshness check read and the verdict it reached.

    The two travel together because the later admission decision is defined
    over both: identity and results are judged only for a document the check
    admitted, and a refusal keeps the category the check chose. Passing the
    observation into :meth:`AdmissionEvidence.admit` makes that dependency an
    argument rather than state this object carries between calls.
    """

    manifest: evidence.Manifest | None
    verdict: policy.Classification


#: The observation a gate holds before it has checked anything. It carries the
#: refusal a skipped check must produce, so a gate that reordered its
#: operations fails closed instead of admitting on no document at all.
MISSING_OBSERVATION = Observation(
    None,
    policy.Classification(
        policy.Outcome.FAILURE, policy.ErrorCategory.MISSING_EVIDENCE
    ),
)


class AdmissionEvidence:
    """Read one manifest and answer the freshness and admission questions."""

    def __init__(
        self,
        configuration: Configuration,
        file_system: filesystem.FileSystem,
        read_wall_clock: WallClock,
    ) -> None:
        """Bind the resolved configuration, the read port, and the clock reader.

        The port is a parameter rather than a lookup so that the only filesystem
        effect these decisions make can be observed, and so a test can hand in a
        document without arranging a directory tree. The clock arrives the same
        way for the same reason: the window is a claim about a reading, and a
        reading the caller cannot control cannot be tested at its boundary.
        """
        self._configuration = configuration
        self._file_system = file_system
        self._read_wall_clock = read_wall_clock

    def check_freshness(self) -> Observation:
        """Read the manifest and refuse every unbounded timestamp.

        The three ways the document can fail to arrive are told apart because an
        operator fixes them differently: nothing at the path is
        ``missing_evidence``, a file the operating system will not read or that
        exceeds the byte bound is ``malformed_evidence``, and a document that
        parsed but lies outside the window is ``stale_evidence``. A parse
        refusal keeps the category the parser chose, so one function decides
        what a malformed document is.

        Returns
        -------
        Observation
            The document read, with success when every moment is inside the
            window, or no document and the category of the first refusal.
        """
        try:
            raw = self._file_system.read_bytes(
                self._configuration.evidence_file, evidence.MAX_EVIDENCE_BYTES
            )
        except FileNotFoundError:
            return Observation(None, _refusal(policy.ErrorCategory.MISSING_EVIDENCE))
        except OSError:
            # Includes `filesystem.EvidenceTooLargeError` and
            # `filesystem.NotRegularFileError`: something is at the path, but it
            # cannot be read as the manifest.
            return Observation(None, _refusal(policy.ErrorCategory.MALFORMED_EVIDENCE))
        parsed = evidence.parse_manifest(raw)
        if parsed.manifest is None:
            return Observation(None, _refusal(parsed.error_category))
        return Observation(parsed.manifest, self._classify_freshness(parsed.manifest))

    def admit(self, observation: Observation) -> policy.Classification:
        """Require the document the check observed to admit the candidate.

        Identity and results are judged first, so a mismatch or an unsuccessful
        canary keeps its own category. An enforcing run then asks whether the
        evidence can be attributed to a trusted producer: a document that is
        readable, fresh, and about this candidate is still refused with
        ``untrusted_evidence`` when the run cannot show who produced it, because
        identity fields agree with the candidate rather than establish origin.
        The window is re-read last, because the bound has to hold at this
        decision and not only at the observation.

        The check's verdict is carried through rather than recomputed: a
        refusal in the observation is final, so only a document the check
        admitted reaches the identity and result checks, and a reading that
        moved back inside the window afterwards cannot revive it. An
        observation with no document reports ``missing_evidence`` rather than
        assuming one: a caller that skipped :meth:`check_freshness` holds
        :data:`MISSING_OBSERVATION`, whose verdict is that refusal, so a
        reordering fails closed instead of admitting on no document at all.

        Parameters
        ----------
        observation
            The document the freshness check read and the verdict it reached.

        Returns
        -------
        policy.Classification
            Success when the document names the candidate, reports every
            required result as successful, is attributable to a trusted
            producer on an enforcing run, and is still inside its window now;
            otherwise the category of the first refusal.
        """
        checked = observation.verdict
        if checked.outcome != policy.Outcome.SUCCESS:
            return checked
        manifest = observation.manifest
        if manifest is None:
            return _refusal(policy.ErrorCategory.MISSING_EVIDENCE)
        configuration = self._configuration
        verdict = evidence.verify(
            manifest,
            evidence.Candidate(
                repository=configuration.repository,
                commit=configuration.revision,
                tag=configuration.tag,
                run_id=configuration.run_id,
            ),
        )
        if verdict.outcome != policy.Outcome.SUCCESS:
            return verdict
        if (
            configuration.enforcement == policy.ADMISSION_ENFORCEMENT_MODE
            and not configuration.producer_trusted
        ):
            return _refusal(policy.ErrorCategory.UNTRUSTED_EVIDENCE)
        return self._classify_freshness(manifest)

    def _classify_freshness(self, manifest: evidence.Manifest) -> policy.Classification:
        """Classify every recorded moment against a wall-clock reading taken now.

        The reading is taken here rather than passed in because both callers ask
        the same question at a different moment, and the moment is what decides
        the answer: a document can be inside its window when it is first
        observed and outside it by the time the gate admits on it.

        Returns
        -------
        policy.Classification
            Success when every moment is inside the window, otherwise the
            refusal the freshness policy names.
        """
        now, is_unusable = self._read_wall_clock()
        if is_unusable:
            return _refusal(policy.ErrorCategory.UNKNOWN)
        return evidence.classify_freshness(
            manifest,
            now=now,
            max_age_seconds=int(self._configuration.max_evidence_age_seconds),
        )


def _refusal(error_category: policy.ErrorCategory) -> policy.Classification:
    """Return the failure classification one bounded category names."""
    return policy.Classification(policy.Outcome.FAILURE, error_category)
