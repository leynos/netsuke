"""Hold the evidence window's two readings to the rule that never revives evidence.

The gate asks about freshness twice: once when it reads the manifest and again
when it is about to admit on it. This module checks that rule across generated
windows rather than at the named boundaries alone. A reading inside the window
is admitted, a reading outside it is refused with ``stale_evidence``, and a
refusal at the first check is never converted into an admission by the second.

The document, the configuration, and the clock are built here rather than taken
from the gate's fixtures, for the reason the fixtures module gives: a document
built from the verifier agrees with a broken verifier. The expected verdict is
computed in Python from the generated values, while the actual verdict comes
from the production decisions, so a drift between them is a counterexample
rather than a case someone forgot to add.

Run via ``make test-release-admission``.
"""

import datetime as dt
import json
import typing as typ

from _release_admission import admission_evidence, configuration, evidence, policy
from hypothesis import given
from hypothesis import strategies as st

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

#: How far a generated reading may sit beyond its window's edges. Wider than the
#: skew allowance, so both reasons for refusing a reading are reachable.
READING_MARGIN_SECONDS = evidence.MAX_EVIDENCE_SKEW_SECONDS + 100
#: The identity the configuration and the manifest agree on.
REPOSITORY = "leynos/netsuke"
REVISION = "a" * 40
TAG = "v1.2.3"
RUN_ID = "1234567890"
#: The results the manifest must report, spelled out here so that a verifier
#: which stopped requiring one cannot agree with this module's document.
REQUIRED_RESULTS = ("history_scan", "release_candidate")


class _ManifestPort:
    """Serve one prepared manifest document and no other filesystem effect."""

    def __init__(self, document: bytes) -> None:
        """Remember the document every read must answer with."""
        self._document = document

    def make_parent(self, _directory: pathlib.Path, /) -> None:
        """Create nothing: the decisions prepare no directory tree."""

    def truncate(self, _configured: str, /) -> None:
        """Create nothing: the decisions write no artefact."""

    def read_bytes(self, _configured: str, limit: int, /) -> bytes:
        """Return the prepared document, honouring the byte bound."""
        return self._document[:limit]


def _moment(epoch_seconds: float) -> str:
    """Return an RFC 3339 UTC spelling of *epoch_seconds*."""
    return (
        dt.datetime
        .fromtimestamp(epoch_seconds, dt.UTC)
        .isoformat()
        .replace("+00:00", "Z")
    )


def _document(issued_at: float) -> bytes:
    """Return a manifest that admits the candidate and is stamped at *issued_at*."""
    moment = _moment(issued_at)
    document = {
        "schema_version": 1,
        "repository": REPOSITORY,
        "commit": REVISION,
        "tag": TAG,
        "workflow_run_id": RUN_ID,
        "issued_at": moment,
        "results": {
            name: {"outcome": "success", "observed_at": moment}
            for name in REQUIRED_RESULTS
        },
    }
    return json.dumps(document).encode("utf-8")


def _configuration(max_age: int) -> configuration.Configuration:
    """Return an enforcing configuration whose window is *max_age* seconds."""
    # The configuration module's own fallback directory, since the decisions
    # build artefact paths from it and this case never writes one.
    temporary = configuration.DEFAULT_TEMPORARY_DIRECTORY
    variables = {
        "GITHUB_REPOSITORY": REPOSITORY,
        "GITHUB_SHA": REVISION,
        "GITHUB_RUN_ID": RUN_ID,
        "RUNNER_TEMP": temporary,
        "NETSUKE_RELEASE_ADMISSION_ENFORCE": policy.ADMISSION_ENFORCEMENT_MODE,
        # This property is about the window, so it states the producer
        # attribution an enforcing run will report once #598 lands. The
        # shipped block that withholds it is pinned separately, by the gate's
        # own producer-trust case and by the workflow contract.
        "NETSUKE_RELEASE_ADMISSION_PRODUCER_TRUSTED": "true",
        "NETSUKE_RELEASE_ADMISSION_TAG": TAG,
        "NETSUKE_RELEASE_ADMISSION_EVIDENCE_FILE": f"{temporary}/case.json",
        "NETSUKE_RELEASE_ADMISSION_MAX_EVIDENCE_AGE_SECONDS": str(max_age),
    }
    return configuration.resolve_configuration(
        variables, configuration.resolve_destination(variables)
    )


def _decide(
    issued_at: float, max_age: int, readings: cabc.Sequence[float]
) -> tuple[policy.Classification, policy.Classification]:
    """Return the freshness check's and the admission decision's verdicts."""
    remaining = list(readings)

    def read_wall_clock() -> tuple[float, bool]:
        """Answer the next scripted reading, in the order the gate asks."""
        if not remaining:
            message = "the gate read the clock more often than this case scripts"
            raise AssertionError(message)
        return remaining.pop(0), False

    gate = admission_evidence.AdmissionEvidence(
        _configuration(max_age),
        _ManifestPort(_document(issued_at)),
        read_wall_clock,
    )
    observation = gate.check_freshness()
    return observation.verdict, gate.admit(observation)


def _inside_window(delta: float, max_age: int) -> bool:
    """Return whether a reading *delta* seconds after the issue instant is fresh.

    The window has two ends: a document older than the maximum age is stale, and
    one stamped further ahead than the skew allowance is refused as well, since
    runner clocks are not synchronised but a large lead names a later release.

    Returns
    -------
    bool
        Whether the gate classifies a document issued *delta* seconds before
        this reading as fresh.
    """
    return delta <= max_age and -delta <= evidence.MAX_EVIDENCE_SKEW_SECONDS


def _reading_offsets(max_age: int) -> st.SearchStrategy[float]:
    """Draw a reading's distance from the issue instant, biased to both edges.

    A uniform draw over the whole span would almost never reach either refusal,
    because both are narrow bands beside a window up to a week wide. One branch
    therefore draws exactly inside the window -- including both of its edges --
    and the other draws the four offsets that sit just outside one of them, so
    a case that needs an inside reading and an outside reading is common rather
    than a rare discovery.

    Returns
    -------
    st.SearchStrategy[float]
        Offsets to add to the issue instant to obtain one scripted reading.
    """
    inside = st.integers(
        min_value=-evidence.MAX_EVIDENCE_SKEW_SECONDS, max_value=max_age
    )
    outside = st.sampled_from((
        max_age + 1,
        max_age + READING_MARGIN_SECONDS,
        -evidence.MAX_EVIDENCE_SKEW_SECONDS - 1,
        -READING_MARGIN_SECONDS,
    ))
    return st.one_of(inside, outside).map(float)


@st.composite
def _window_case(draw: st.DrawFn) -> tuple[float, int, float, float]:
    """Draw the issue instant, the window length, and the two clock readings."""
    issued_at = float(draw(st.integers(min_value=10**9, max_value=2 * 10**9)))
    # The widest window is the policy's own ceiling, so no case names a window
    # the gate would refuse outright.
    max_age = draw(
        st.integers(min_value=0, max_value=evidence.MAX_EVIDENCE_MAX_AGE_SECONDS)
    )
    offsets = _reading_offsets(max_age)
    return issued_at, max_age, issued_at + draw(offsets), issued_at + draw(offsets)


@given(case=_window_case())
def test_a_reading_outside_the_window_is_never_admitted(
    case: tuple[float, int, float, float],
) -> None:
    """Require both decisions to refuse exactly when their reading is outside."""
    issued_at, max_age, first, second = case
    checked, admitted = _decide(issued_at, max_age, [first, second])
    first_is_fresh = _inside_window(first - issued_at, max_age)
    second_is_fresh = _inside_window(second - issued_at, max_age)

    assert (checked.outcome == policy.Outcome.SUCCESS) == first_is_fresh, (
        "the check must admit exactly the documents inside its window; "
        f"issued_at={issued_at}, max_age={max_age}, reading={first}"
    )
    assert (admitted.outcome == policy.Outcome.SUCCESS) == (
        first_is_fresh and second_is_fresh
    ), (
        "admission must require both readings inside the window; "
        f"issued_at={issued_at}, max_age={max_age}, readings={first}, {second}"
    )
    assert admitted.outcome != policy.Outcome.SUCCESS or (
        checked.outcome == policy.Outcome.SUCCESS
    ), (
        "an admission must never revive evidence the check refused; "
        f"issued_at={issued_at}, max_age={max_age}, readings={first}, {second}"
    )
    if first_is_fresh and not second_is_fresh:
        assert admitted.error_category == policy.ErrorCategory.STALE_EVIDENCE, (
            "expiry at the decision must keep its own category; "
            f"issued_at={issued_at}, max_age={max_age}, readings={first}, {second}"
        )
