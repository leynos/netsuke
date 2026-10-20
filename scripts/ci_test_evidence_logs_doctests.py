"""Parse target-scoped doctest batches from saved Cargo logs.

Cargo may announce the next target before a compile-fail batch ends, so this
module keeps active and completed target identities as explicit state.
"""

import dataclasses as dc
import re
import typing as typ

if typ.TYPE_CHECKING:
    from ci_test_evidence_types import DoctestEvidence, DoctestSection

RESULT = re.compile(
    r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;"
)


def closes_doctest_scope(body: str, target: str | None, completed: str | None) -> bool:
    """Close a command or completed target while retaining a new target.

    For example, ``closes_doctest_scope("$ cargo test", None, None)`` is true.

    Returns
    -------
    bool
        Whether the line closes the active command or target scope.
    """
    boundary = body.startswith(("##[endgroup]", "$ ", "[command]"))
    finished_target = body.startswith("all doctests ran in ") and target == completed
    return boundary or finished_target


def doctest_records(lines: list[tuple[str | None, str]]) -> DoctestEvidence:
    """Collect target-scoped doctest counts and timing from Cargo output.

    Parameters
    ----------
    lines
        Timestamp and text pairs from one saved job log.

    Returns
    -------
    DoctestEvidence
        Completed target sections and aggregate counts; absent result markers
        leave counts unknown.
    """
    accumulator = _DoctestAccumulator()
    for timestamp, body in lines:
        accumulator.consume(timestamp, body)
    return accumulator.finish()


@dc.dataclass(slots=True)
class _DoctestAccumulator:
    """Collect Cargo's split doctest batches without losing target identity."""

    sections: list[DoctestSection] = dc.field(default_factory=list)
    current: DoctestSection | None = None
    target: str | None = None
    completed_target: str | None = None

    def consume(self, timestamp: str | None, body: str) -> None:
        """Apply one native log line to the active doctest section."""
        if closes_doctest_scope(body, self.target, self.completed_target):
            self.reset_scope()
        self.announce_target(body)
        self.start_batch(timestamp, body)
        self.record_result(timestamp, body)

    def reset_scope(self) -> None:
        """Clear state when a Cargo command or target has finished."""
        self.target, self.current, self.completed_target = None, None, None

    def announce_target(self, body: str) -> None:
        """Remember the target named by a ``Doc-tests`` marker."""
        marker = re.fullmatch(r"\s*Doc-tests ([A-Za-z0-9_-]+)", body)
        if marker:
            self.target = marker[1]

    def start_batch(self, timestamp: str | None, body: str) -> None:
        """Start a target batch when Cargo announces tests to execute."""
        starts_batch = re.fullmatch(r"running \d+ tests?", body) is not None
        if self.target is None or self.current is not None:
            return
        if not starts_batch and not _body_matches_doctest_target(body):
            return
        self.current = {
            "target": self.target,
            "started_at": timestamp,
            "ended_at": None,
            "counts": {},
        }

    def record_result(self, timestamp: str | None, body: str) -> None:
        """Finish one batch while retaining later compile-fail batches."""
        if self.current is None:
            return
        result = RESULT.match(body)
        if result is None:
            return
        self.current.update(
            ended_at=timestamp,
            counts=dict(
                zip(
                    ("passed", "failed", "ignored"),
                    map(int, result.groups()),
                    strict=True,
                )
            ),
        )
        self.sections.append(self.current)
        self.completed_target = self.current["target"]
        self.current = None

    def finish(self) -> DoctestEvidence:
        """Return the observed sections and aggregate counters."""
        return {"counts": _doctest_counts(self.sections), "sections": self.sections}


def _body_matches_doctest_target(body: str) -> bool:
    """Identify a target marker that itself starts an empty test batch."""
    return re.fullmatch(r"\s*Doc-tests [A-Za-z0-9_-]+", body) is not None


def _doctest_counts(sections: list[DoctestSection]) -> dict[str, int] | None:
    """Sum native counts, returning unknown when no target completed."""
    if not sections:
        return None
    return {
        key: sum(section["counts"][key] for section in sections)
        for key in ("passed", "failed", "ignored")
    }
