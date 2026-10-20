"""Parse Nextest-specific records for the offline CI evidence collector."""

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
    result = empty_evidence()["nextest"]
    attempts: dict[str, TestEvidence] = {}
    summarised = False
    for timestamp, body in lines:
        start = STARTING.fullmatch(body)
        if start:
            attempts.clear()
            summarised = False
            _record_start(result, start)
        if not summarised:
            _record_test(result, attempts, timestamp, body)
        summary = SUMMARY.fullmatch(body)
        if summary:
            summarised = True
            _record_summary(result, summary)
    result["tests"] = list(attempts.values())
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
    return result


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


def _record_start(result: NextestEvidence, start: re.Match[str]) -> None:
    """Record selected and discovered counts from a Nextest start line."""
    selected = int(start["selected"])
    skipped_text = start["skipped_tests"]
    if skipped_text is not None:
        skipped = int(skipped_text)
    elif start["skipped_binaries"] is None:
        skipped = 0
    else:
        skipped = None
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
