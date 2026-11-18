"""Record each smoke phase's outcome as the run walks through it.

A smoke run must leave evidence even when it is killed part-way. The ledger
rewrites two artefacts on every transition: the bounded verdict record a
consumer can read afterwards, and the bounded phase log the workflow turns into
one triple of job outputs per phase. Both carry fixed phase labels only, so
nothing an input, an asset name, or an error string says can reach them.

Each phase is recorded as ``failure`` *before* its work begins and raised to
``success`` only once the work returns. A run that dies mid-phase therefore
names the phase that was in flight, which is what the job telemetry needs to
distinguish "the archive was corrupt" from "the runner vanished". A phase that
refuses records ``failure`` a second time with its end timestamp before the
refusal propagates, so a completed refusal carries the same timing pair a
success does.

Run through ``scripts/consumer_smoke_cli.py``; behavioural coverage lives in
``scripts/tests/test_consumer_smoke_cli.py``.
"""

import contextlib
import dataclasses
import time
import typing as typ

import consumer_smoke_evidence as evidence
import release_consumer_payload as payload
from release_consumer_smoke import ConsumerSmokeError

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib


class VerdictWriteError(ConsumerSmokeError):
    """Report that the phase-and-verdict artefacts could not be written."""


@dataclasses.dataclass(slots=True)
class PhaseLedger:
    """Record each phase's outcome and keep both artefacts current.

    Parameters
    ----------
    destination
        The file the bounded verdict record is rewritten into.
    log_path
        The file the bounded phase log is rewritten into.
    clock
        The nanosecond clock used to timestamp each transition. Injected so a
        test can assert the recorded spans without waiting for them.

    Notes
    -----
    Both artefacts are rewritten on every transition, so a reader never has to
    correlate two files that disagree about which phase was in flight.
    """

    destination: pathlib.Path
    log_path: pathlib.Path
    clock: cabc.Callable[[], int] = time.time_ns
    outcomes: dict[str, str] = dataclasses.field(default_factory=dict)
    log: dict[str, dict[str, object]] = dataclasses.field(default_factory=dict)

    def start_fresh(self) -> None:
        """Remove both artefacts, so nothing a previous run left survives.

        Notes
        -----
        A run refused before its first phase writes nothing, so without this it
        would leave an earlier run's verdict where a consumer expects this
        run's. A path that cannot be removed is left for the write that follows
        to report, because an artefact that cannot be deleted is also one this
        run cannot rewrite.
        """
        for path in (self.destination, self.log_path):
            with contextlib.suppress(OSError):
                path.unlink(missing_ok=True)

    def run[T](self, label: str, action: cabc.Callable[[], T]) -> T:
        """Run one phase under its label and return its result.

        Parameters
        ----------
        label
            One of :data:`release_consumer_payload.SMOKE_PHASES`.
        action
            The work the phase performs.

        Returns
        -------
        T
            Whatever ``action`` returned.

        Notes
        -----
        A ledger that cannot write its own artefacts fails as
        :class:`VerdictWriteError`, raised by the write this method performs.
        """
        started_ns = self.clock()
        # Recorded as failed before the work starts, so a run killed mid-phase
        # still names the phase in flight and leaves its start time behind.
        self._record(label, "failure", started_ns, None)
        try:
            result = action()
        except Exception:
            # A phase that refuses has finished, so it reports an end time even
            # though its outcome stays ``failure``. An interrupted run never
            # reaches here and keeps the start-only record above, which is how
            # a refusal is told apart from a runner that vanished.
            self._record(label, "failure", started_ns, self.clock())
            raise
        self._record(label, "success", started_ns, self.clock())
        return result

    def _record(
        self, label: str, outcome: str, started_ns: int, ended_ns: int | None
    ) -> None:
        """Record one transition and rewrite both artefacts.

        Notes
        -----
        An artefact that cannot be written fails as
        :class:`VerdictWriteError`, raised by the write this method performs.
        """
        self.outcomes[label] = outcome
        self.log[label] = {
            "outcome": outcome,
            "started_ns": started_ns,
            "ended_ns": ended_ns,
        }
        self._write()

    def _write(self) -> None:
        """Rewrite both artefacts from the outcomes observed so far.

        Raises
        ------
        VerdictWriteError
            If either artefact cannot be written. A run whose telemetry cannot
            be recorded is a wiring fault, so it stops instead of finishing
            silently without evidence.
        """
        verdict = payload.verdict_record(self.outcomes)
        try:
            evidence.write_json(self.destination, verdict)
            evidence.write_json(self.log_path, phase_log_document(self.log))
        except OSError as error:
            msg = (
                f"the verdict artefacts {self.destination} and {self.log_path} "
                f"could not be written: {error}"
            )
            raise VerdictWriteError(msg) from error


def phase_log_document(
    log: cabc.Mapping[str, cabc.Mapping[str, object]],
) -> dict[str, object]:
    """Render the bounded phase log the workflow turns into job outputs.

    Returns
    -------
    dict[str, object]
        One entry per registered phase that has a transition, and nothing else.

    Examples
    --------
    >>> phase_log_document({"crate_download": {"outcome": "success"}})
    {'crate_download': {'outcome': 'success'}}
    >>> phase_log_document({"a_phase_no_contract_names": {"outcome": "success"}})
    {}
    """
    return {label: dict(log[label]) for label in payload.SMOKE_PHASES if label in log}
