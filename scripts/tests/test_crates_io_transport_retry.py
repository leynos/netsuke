"""Pin the bounded archive-polling schedule and its status policy.

``test_crates_io_transport_properties.py`` covers the registry boundary and the
upload frame. This module covers the retry loop itself: how many requests one
poll may issue, when it stops early, and where the retryable window closes.
It lives apart so each module stays within the repository's file-length cap.
"""

import urllib.error
import urllib.request

import pytest
from crates_io_transport_http import (
    DownloadPollPolicy,
    _download_status_class,
    _retryable_download_status,
    poll_archive,
)
from crates_io_transport_support import (
    FakeOpener,
    FakeResponse,
    http_error_response,
)
from hypothesis import given, settings
from hypothesis import strategies as st

ARCHIVE_URL = "https://static.crates.io/crates/netsuke-build/netsuke-build-1.2.3.crate"

RETRYABLE_404 = "retryable-404"
RETRYABLE_503 = "retryable-503"
TERMINAL_200 = "terminal-200"
TERMINAL_422 = "terminal-422"
OFFLINE = "offline"
TERMINAL_OUTCOMES = frozenset({TERMINAL_200, TERMINAL_422})


def _outcome(name: str) -> FakeResponse | BaseException:
    """Build the configured transport outcome for one generated poll attempt."""
    outcomes: dict[str, FakeResponse | BaseException] = {
        RETRYABLE_404: http_error_response(ARCHIVE_URL, 404, b"absent"),
        RETRYABLE_503: http_error_response(ARCHIVE_URL, 503, b"busy"),
        TERMINAL_200: FakeResponse(200, b"archive"),
        TERMINAL_422: http_error_response(ARCHIVE_URL, 422, b"rejected"),
        OFFLINE: urllib.error.URLError("offline"),
    }
    return outcomes[name]


def _poll(
    attempts: int, names: list[str]
) -> tuple[list[urllib.request.Request], list[float], int]:
    """Run one bounded poll and return its requests, delays, and attempt count."""
    request = urllib.request.Request(ARCHIVE_URL)
    # An empty opener queue would surface as IndexError, so repeat the generated
    # sequence until it can exhaust the largest permitted budget.
    opener = FakeOpener(*[_outcome(name) for name in names * attempts])
    delays: list[float] = []

    result = poll_archive(
        opener, request, DownloadPollPolicy(attempts, 0.0, 1.0), delays.append
    )
    return [request for request, _timeout in opener.requests], delays, result.attempts


@settings(max_examples=100, deadline=None, derandomize=True)
@given(
    attempts=st.integers(min_value=1, max_value=6),
    names=st.lists(
        st.sampled_from((
            RETRYABLE_404,
            RETRYABLE_503,
            TERMINAL_200,
            TERMINAL_422,
            OFFLINE,
        )),
        min_size=1,
        max_size=12,
    ),
)
def test_poll_archive_honours_its_budget_and_stops_on_the_first_terminal_response(
    attempts: int, names: list[str]
) -> None:
    """Stop at the first terminal outcome, or at the attempt budget.

    Three things tie the observable call schedule to the policy: no more
    requests than permitted, one delay between every pair of consecutive
    requests and none after the last, and an immediate stop when the very first
    response is terminal.
    """
    _requests, delays, reported_attempts = _poll(attempts, names)

    assert 1 <= reported_attempts <= attempts, "never exceed the attempt budget"
    assert len(delays) == max(0, reported_attempts - 1), (
        "wait exactly between consecutive requests, never after the last"
    )
    if names[0] in TERMINAL_OUTCOMES:
        assert reported_attempts == 1, "a terminal response ends the poll at once"


@pytest.mark.parametrize(
    ("status", "expected_retryable"),
    [
        pytest.param(200, False, id="success"),
        pytest.param(301, False, id="redirect"),
        pytest.param(403, True, id="forbidden-counts-as-absent"),
        pytest.param(404, True, id="absent"),
        pytest.param(410, False, id="gone"),
        pytest.param(422, False, id="unprocessable"),
        pytest.param(499, False, id="client-error-edge"),
        pytest.param(500, True, id="server-error-start"),
        pytest.param(599, True, id="server-error-edge"),
        pytest.param(600, False, id="past-the-status-range"),
        pytest.param(0, False, id="absent-status"),
    ],
)
def test_retryable_status_window_covers_absence_and_server_errors_only(
    status: int, *, expected_retryable: bool
) -> None:
    """Retry absence and 5xx responses, and stop on every other status."""
    assert _retryable_download_status(status) is expected_retryable, (
        f"status {status} retryability"
    )


@pytest.mark.parametrize(
    ("status", "status_class"),
    [
        pytest.param(100, "1xx", id="informational"),
        pytest.param(206, "2xx", id="partial-success"),
        pytest.param(404, "4xx", id="client-error"),
        pytest.param(503, "5xx", id="retryable"),
        pytest.param(99, "unknown", id="below-the-range"),
        pytest.param(600, "unknown", id="past-the-range"),
        pytest.param(0, "unknown", id="sentinel"),
    ],
)
def test_status_classification_reports_only_bounded_labels(
    status: int, status_class: str
) -> None:
    """Report a fixed status class so the report keeps low cardinality."""
    assert _download_status_class(status) == status_class, (
        f"status {status} must classify as {status_class!r}"
    )


def test_poll_archive_keeps_the_first_non_retryable_response() -> None:
    """Return the bytes of the response that ended polling early."""
    request = urllib.request.Request(ARCHIVE_URL)
    opener = FakeOpener(
        http_error_response(ARCHIVE_URL, 503, b"busy"),
        http_error_response(ARCHIVE_URL, 422, b"rejected"),
        FakeResponse(200, b"never reached"),
    )

    result = poll_archive(
        opener, request, DownloadPollPolicy(3, 0.0, 1.0), lambda _delay: None
    )

    assert result.response == (422, b"rejected"), "retain the terminating response"
    assert result.status_class == "4xx", "classify the terminating status"
    assert result.last_error is None, "a complete response is not a transport error"
    assert len(opener.requests) == 2, "the later queued response is never requested"
