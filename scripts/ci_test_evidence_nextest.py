"""Parse Nextest start, test-result, and summary records from saved CI logs.

The offline CI evidence collector uses this adapter for selected and executed
counts, observed retries, final per-test outcomes, and native durations. A new
``Starting`` line begins a run; records after a matching ``Summary`` are ignored
until another run begins.

Examples
--------
>>> lines = [(None, "Starting 1 test across 1 binary"),
...          (None, "PASS [ 0.1s] fixture::unit works"),
...          (None, "Summary [ 0.1s] 1 test run: 1 passed")]
>>> parse_nextest_records(lines)["executed_count"]
1
"""

import dataclasses as dc
import re

from ci_test_evidence_types import NextestEvidence, TestEvidence, empty_evidence

TEST = re.compile(
    r"\s*(?:TRY (?P<try_number>\d+) )?"
    r"(?P<outcome>PASS|FAIL|SKIP|TIMEOUT|LEAK|ABORT|SIG[A-Z0-9]+) \[\s*"
    r"(?P<seconds>\d{1,9}(?:\.\d{1,9})?s)?\s*\] "
    r"(?:\(\s*\d+/\d+\) )?"
    r"(?P<binary>[A-Za-z0-9_:/.-]+) (?P<name>[A-Za-z0-9_:.+-]+)$"
)
SUMMARY = re.compile(
    r"\s*Summary \[\s*(\d{1,9}(?:\.\d{1,9})?)s\] (\d+)(?:/\d+)? "
    r"tests? run: (.*)$"
)
STARTING = re.compile(
    r"\s*Starting (?P<selected>\d+) tests? across \d+ binar(?:y|ies)"
    r"(?: \((?:(?P<skipped_tests>\d+) tests?"
    r"(?: and \d+ binar(?:y|ies))?|"
    r"(?P<skipped_binaries>\d+) binar(?:y|ies)) skipped"
    r"(?: via [^)]+)?\))?"
)


def parse_nextest_records(
    lines: list[tuple[str | None, str]],
) -> NextestEvidence:
    """Retain final test outcomes and the latest observed Nextest run.

    Parameters
    ----------
    lines
        Timestamp and body pairs from a saved CI log.

    Returns
    -------
    NextestEvidence
        Native timing, counts, and final outcomes. The executed identity
        inventory is complete only when one recorded identity exists for each
        test reported by the summary.

    Examples
    --------
    >>> lines = [(None, "Starting 1 test across 1 binary"),
    ...          (None, "PASS [ 0.1s] fixture::unit works"),
    ...          (None, "Summary [ 0.1s] 1 test run: 1 passed")]
    >>> parse_nextest_records(lines)["executed_identity_inventory_complete"]
    True
    """
    accumulator = _NextestAccumulator()
    for timestamp, body in lines:
        accumulator.consume(timestamp, body)
    return accumulator.finish()


@dc.dataclass(slots=True)
class _NextestAccumulator:
    """Keep the active run's summary and identities together while parsing."""

    result: NextestEvidence = dc.field(
        default_factory=lambda: empty_evidence()["nextest"]
    )
    attempts: dict[str, TestEvidence] = dc.field(default_factory=dict)
    summarised: bool = False

    def consume(self, timestamp: str | None, body: str) -> None:
        """Apply one native line and retain the latest run's state."""
        start = STARTING.fullmatch(body)
        if start:
            self.attempts.clear()
            self.summarised = False
            _record_start(self.result, start)
        if not self.summarised:
            _record_test(self.result, self.attempts, timestamp, body)
        summary = SUMMARY.fullmatch(body)
        if summary:
            _record_summary(self.result, summary)
            self.summarised = True

    def finish(self) -> NextestEvidence:
        """Return the final run with its identity completeness flags."""
        self.result["tests"] = list(self.attempts.values())
        _record_identity_completeness(self.result, self.attempts)
        return self.result


def _record_identity_completeness(
    result: NextestEvidence, attempts: dict[str, TestEvidence]
) -> None:
    """Compare recorded test identities with the final run summary."""
    executed_attempts = [
        test for test in attempts.values() if test["outcome"] != "skip"
    ]
    result["executed_identity_inventory_complete"] = (
        result["executed_count"] is not None
        and len(executed_attempts) == result["executed_count"]
    )
    result["identity_inventory_complete"] = (
        result["executed_identity_inventory_complete"]
        and result["skipped_count"] == 0
        and result["selected_count"] == result["executed_count"]
    )


def nextest_phase_marker(body: str) -> str | None:
    """Identify a Nextest start or summary line without parsing its counts.

    Parameters
    ----------
    body
        One timestamp-free line from a saved CI log.

    Returns
    -------
    str | None
        ``"nextest_started"`` or ``"nextest_completed"`` for recognized
        markers, otherwise ``None``.

    Examples
    --------
    >>> nextest_phase_marker("Starting 1 test across 1 binary")
    'nextest_started'
    """
    if STARTING.fullmatch(body):
        return "nextest_started"
    if SUMMARY.fullmatch(body):
        return "nextest_completed"
    return None


def _skipped_test_count(start: re.Match[str]) -> int | None:
    """Return skipped tests, or ``None`` when only binaries are reported."""
    skipped_text = start["skipped_tests"]
    if skipped_text is not None:
        return int(skipped_text)
    return 0 if start["skipped_binaries"] is None else None


def _record_start(result: NextestEvidence, start: re.Match[str]) -> None:
    """Record selected and discovered counts from a Nextest start line."""
    selected = int(start["selected"])
    skipped = _skipped_test_count(start)
    result.update(
        seconds=None,
        selected_count=selected,
        discovered_count=selected + skipped if skipped is not None else None,
        executed_count=None,
        skipped_count=skipped,
        outcomes=None,
        identity_inventory_complete=False,
        executed_identity_inventory_complete=False,
        retries=None,
    )


def _record_test(
    result: NextestEvidence,
    attempts: dict[str, TestEvidence],
    timestamp: str | None,
    body: str,
) -> None:
    """Retain the latest attempt for a test and count only observed retries."""
    test = TEST.fullmatch(body)
    if test is None:
        return
    seconds = test["seconds"]
    outcome = test["outcome"].lower()
    if seconds is None and outcome != "skip":
        return
    identity = f"{test['binary']} {test['name']}"
    attempts[identity] = {
        "identity": identity,
        "outcome": outcome,
        "seconds": float(seconds.removesuffix("s")) if seconds else None,
        "ended_at": timestamp,
    }
    try_number = test["try_number"]
    if try_number is not None:
        result["retries"] = (result["retries"] or 0) + int(int(try_number) > 1)


def _record_summary(result: NextestEvidence, summary: re.Match[str]) -> None:
    """Record elapsed time, executed count and outcomes from a summary line."""
    result.update(
        seconds=float(summary[1]),
        executed_count=int(summary[2]),
        outcomes={
            label: int(count)
            for count, label in re.findall(
                r"(\d+) (passed|failed|skipped|timed out|leaky)", summary[3]
            )
        },
    )
