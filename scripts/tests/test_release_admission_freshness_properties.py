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
import hashlib
import json
import typing as typ

from _release_admission import admission_evidence, configuration, evidence, policy
from hypothesis import given
from hypothesis import strategies as st
from release_admission_evidence_fixtures import (
    REQUIRED_RESULT_NAMES as REQUIRED_RESULTS,
)

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


def _configuration(max_age: int, document: bytes) -> configuration.Configuration:
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
        # This property is about the window, so it supplies the digest the
        # producer job publishes for this document. Attribution itself is
        # pinned separately, by the gate's own producer-trust cases and by the
        # workflow contract.
        "NETSUKE_RELEASE_ADMISSION_EVIDENCE_SHA256": hashlib.sha256(
            document
        ).hexdigest(),
        "NETSUKE_RELEASE_ADMISSION_TAG": TAG,
        "NETSUKE_RELEASE_ADMISSION_EVIDENCE_FILE": f"{temporary}/case.json",
        "NETSUKE_RELEASE_ADMISSION_MAX_EVIDENCE_AGE_SECONDS": str(max_age),
    }
    return configuration.resolve_configuration(
        variables, configuration.resolve_destination(variables)
    )


def _decide(
    issued_at: float, max_age: int, readings: cabc.Sequence[tuple[float, bool]]
) -> tuple[policy.Classification, policy.Classification]:
    """Return the freshness check's and the admission decision's verdicts.

    Each scripted reading is the ``(seconds, is_unusable)`` pair the gate's own
    clock seam returns, because the flag rather than the number is what carries
    "this is not a moment". Scripting a bare number would leave every case
    unable to express a reading the gate must refuse.

    Returns
    -------
    tuple[policy.Classification, policy.Classification]
        The freshness check's verdict and the admission decision's verdict, in
        the order the gate reaches them.
    """
    remaining = list(readings)

    def read_wall_clock() -> tuple[float, bool]:
        """Answer the next scripted reading, in the order the gate asks."""
        if not remaining:
            message = "the gate read the clock more often than this case scripts"
            raise AssertionError(message)
        return remaining.pop(0)

    document = _document(issued_at)
    gate = admission_evidence.AdmissionEvidence(
        _configuration(max_age, document),
        _ManifestPort(document),
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
    checked, admitted = _decide(issued_at, max_age, [(first, False), (second, False)])
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


def test_an_unusable_reading_refuses_rather_than_name_an_instant() -> None:
    """Refuse both decisions when the clock reports a reading it cannot use.

    An unusable reading arrives as the same zero fallback a reading of the
    epoch does, so only the flag can tell them apart: a gate that read the
    number alone would judge every document against 1970 and refuse it as
    stale, which is the right verdict for the wrong reason and the wrong
    category for an operator to act on. The first reading is asked when the
    check observes the document and the second when admission re-reads the
    window, so the case scripts each position separately and requires the
    ``unknown`` category at both.
    """
    issued_at = 1_700_000_000.0
    max_age = 60
    unchecked, _ = _decide(issued_at, max_age, [(0.0, True), (issued_at, False)])
    assert (unchecked.outcome, unchecked.error_category) == (
        policy.Outcome.FAILURE,
        policy.ErrorCategory.UNKNOWN,
    ), "an unusable reading at the check must refuse it with the unknown category"

    observed, admitted = _decide(issued_at, max_age, [(issued_at, False), (0.0, True)])
    assert observed.outcome == policy.Outcome.SUCCESS, (
        "the second reading belongs to the decision, so the check must pass first"
    )
    assert (admitted.outcome, admitted.error_category) == (
        policy.Outcome.FAILURE,
        policy.ErrorCategory.UNKNOWN,
    ), "an unusable reading at the decision must refuse the admission"
