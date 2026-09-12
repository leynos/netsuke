"""Publish idempotent, bounded GitHub Check Runs for trusted workflow helpers.

Only ``coverage_pr_submission.py`` may construct this module's adapter in
production. Publication is one explicit, fallible operation that owns its
idempotency lookup, so no caller can issue a Check Run query that reaches
GitHub on its own. Every request crosses a narrow transport port: the
production transport confines ``urllib`` and the step-local token, while tests
inject a recording transport or a local contract origin.
"""

import dataclasses
import json
import typing as typ
import urllib.parse
import urllib.request

if typ.TYPE_CHECKING:
    import collections.abc as cabc

CHECK_RUN_NAME = "CodeScene coverage"
# Ask for every Check Run on the commit, not just the newest one per name. The
# documented default, ``latest``, would hide an existing run this publisher
# must update, and the publication would then create a duplicate Check Run.
CHECK_RUN_FILTER = "all"
# The documented page-size default, stated explicitly so the response stays
# inside ``MAXIMUM_RESPONSE_BYTES`` and the page arithmetic below is fixed.
CHECK_RUN_PAGE_SIZE = 30
# The lookup endpoint returns the Check Runs of at most the 1000 most recent
# check suites on one ref, so these pages cover its whole documented ceiling.
MAXIMUM_CHECK_RUN_PAGES = 34
GITHUB_API_ORIGIN = "https://api.github.com"
GITHUB_API_VERSION = "2022-11-28"
MAXIMUM_RESPONSE_BYTES = 64 * 1024


class CheckRunPublicationError(RuntimeError):
    """Describe a rejected or oversized GitHub Check Run publication."""

    @classmethod
    def malformed_response(cls) -> typ.Self:
        """Build the fixed malformed-response diagnostic."""
        return cls("GitHub Check Runs response is malformed")

    @classmethod
    def response_is_not_json(cls) -> typ.Self:
        """Build the fixed non-JSON response diagnostic."""
        return cls("GitHub Check Runs response is not JSON")

    @classmethod
    def response_exceeds_limit(cls) -> typ.Self:
        """Build the fixed response-size diagnostic."""
        return cls("GitHub Check Run response exceeds limit")

    @classmethod
    def missing_payload_text(cls, name: str) -> typ.Self:
        """Build the fixed missing-payload diagnostic for one field."""
        return cls(f"Check Run payload lacks {name}")

    @classmethod
    def lookup_exceeds_limit(cls) -> typ.Self:
        """Build the fixed over-limit identity-lookup diagnostic."""
        return cls("GitHub Check Runs lookup exceeds limit")


@dataclasses.dataclass(frozen=True, slots=True)
class CheckRunRequest:
    """Describe one bounded GitHub Check Runs API request.

    Attributes
    ----------
    method
        HTTP method the transport must use.
    path
        Absolute path on the GitHub API origin, beginning with ``/repos/``.
    payload
        Bounded payload for a write request, or ``None`` for the lookup read.
    """

    method: str
    path: str
    payload: cabc.Mapping[str, object] | None = None


class CheckRunTransport(typ.Protocol):
    """Send one bounded Check Run API request and return its limited response.

    This is the publication boundary's driven port. The publisher selects
    requests from bounded payload data; a transport adapter owns the process's
    only network access, its credentials, and its bounded response reads.
    """

    def __call__(self, request: CheckRunRequest) -> bytes:
        """Return the raw response bytes for one bounded API request."""


class GitHubApiTransport:
    """Send Check Run API requests to GitHub with the step-local token.

    Parameters
    ----------
    token
        Step-local GitHub token used only in API request headers.
    api_origin
        Fixed GitHub API origin in production. Tests may supply a local
        contract server to exercise the same protocol without credentials.

    Notes
    -----
    Each response is read as at most one byte beyond
    ``MAXIMUM_RESPONSE_BYTES``, so an oversized reply is rejected before it
    can consume runner memory.
    """

    def __init__(self, token: str, api_origin: str = GITHUB_API_ORIGIN) -> None:
        """Store the credential and fixed API origin for later requests."""
        self._token = token
        self._api_origin = api_origin.rstrip("/")

    def __call__(self, request: CheckRunRequest) -> bytes:
        """Send one bounded request and return at most the response limit."""
        data = _encoded_payload(request.payload)
        endpoint = f"{self._api_origin}{request.path}"
        # ruff:ignore[suspicious-url-open-usage] -- the endpoint is the fixed GitHub API host; tests inject only a local contract server.
        outbound = urllib.request.Request(
            endpoint,
            data=data,
            headers={
                "Accept": "application/vnd.github+json",
                "Authorization": f"Bearer {self._token}",
                "X-GitHub-Api-Version": GITHUB_API_VERSION,
            },
            method=request.method,
        )
        # ruff:ignore[suspicious-url-open-usage] -- the endpoint is the fixed GitHub API host; tests inject only a local contract server.
        with urllib.request.urlopen(outbound, timeout=30) as response:
            response_bytes = response.read(MAXIMUM_RESPONSE_BYTES + 1)
        if len(response_bytes) > MAXIMUM_RESPONSE_BYTES:
            raise CheckRunPublicationError.response_exceeds_limit()
        return response_bytes


class GitHubCheckRunPublisher:
    """Publish one Check Run idempotently through an injected transport.

    Parameters
    ----------
    repository
        Trusted owner-and-repository identifier supplied by GitHub Actions.
    transport
        Narrow request transport that owns network access and bounded reads.

    Attributes
    ----------
    repository
        Trusted repository whose Check Runs this adapter publishes.

    Notes
    -----
    The originating workflow run ID is the idempotency key: a matching Check
    Run is updated, otherwise one is created. Workflow-level concurrency
    serializes the same key before this API protocol runs, and the identity
    lookup reads every Check Run on the originating commit rather than only
    the newest per name, so an already-published run is always found.
    """

    def __init__(self, repository: str, transport: CheckRunTransport) -> None:
        """Store the trusted repository and its bounded request transport."""
        self.repository = repository
        self._transport = transport

    def publish(self, payload: dict[str, object]) -> None:
        """Create or update the Check Run selected by its bounded identity.

        This is the operation the callable publisher port binds to, so one
        call publishes exactly one completed Check Run.

        Parameters
        ----------
        payload
            Completed Check Run payload containing the trusted commit SHA and
            originating workflow run ID.

        Notes
        -----
        A missing payload field, a malformed or non-JSON lookup response, or a
        response above the read limit fails the publication with
        ``CheckRunPublicationError`` from the identity lookup, the payload
        helpers, or the transport, so no partial Check Run is written.
        """
        check_run_id = self._matching_check_run_id(payload)
        method = "PATCH" if check_run_id is not None else "POST"
        path = self._check_run_path(check_run_id)
        self._transport(CheckRunRequest(method, path, payload))

    def _check_run_path(self, check_run_id: int | None = None) -> str:
        """Return the fixed Check Runs path, optionally for one update."""
        path = f"/repos/{self.repository}/check-runs"
        return path if check_run_id is None else f"{path}/{check_run_id}"

    def _check_run_lookup_path(self, commit_sha: str, page_number: int) -> str:
        """Return the fixed one-page lookup path for a commit's Check Runs."""
        query = urllib.parse.urlencode({
            "check_name": CHECK_RUN_NAME,
            "filter": CHECK_RUN_FILTER,
            "per_page": CHECK_RUN_PAGE_SIZE,
            "page": page_number,
        })
        return f"/repos/{self.repository}/commits/{commit_sha}/check-runs?{query}"

    def _matching_check_run_id(self, payload: cabc.Mapping[str, object]) -> int | None:
        """Return the ID of this publisher's already-published Check Run.

        Parameters
        ----------
        payload
            Completed Check Run payload carrying the trusted commit SHA and
            originating workflow run ID that identify the Check Run.

        Returns
        -------
        int | None
            ID of the already-published Check Run, or ``None`` when the
            originating commit has no Check Run with this name and identity.

        Notes
        -----
        The lookup is an internal step of :meth:`publish` rather than an
        independent operation: it exists only to make publication idempotent
        and reaches the network only through the injected transport. It asks
        for every Check Run on the commit and then follows the server's page
        total, so an existing run is found however far down the list it sits;
        concluding early would create the duplicate this lookup exists to
        prevent. A lookup response that is not a JSON object carrying a
        bounded ``total_count`` and a ``check_runs`` list fails the publication
        with ``CheckRunPublicationError``, as does a total beyond the pages
        ``MAXIMUM_CHECK_RUN_PAGES`` bounds: a lookup that cannot be exhaustive
        must not publish a possible duplicate.
        """
        commit_sha = _required_payload_text(payload, "head_sha")
        external_id = _required_payload_text(payload, "external_id")
        page_number = 1
        while True:
            response = self._transport(
                CheckRunRequest(
                    "GET",
                    self._check_run_lookup_path(commit_sha, page_number),
                )
            )
            response_body = _response_mapping(response)
            check_run_id = _matching_id_in_page(response_body, external_id)
            if check_run_id is not None:
                return check_run_id
            if not _has_next_page(response_body, page_number):
                return None
            page_number += 1


def _encoded_payload(payload: cabc.Mapping[str, object] | None) -> bytes | None:
    """Encode one bounded request payload, or return ``None`` for a read."""
    return None if payload is None else json.dumps(payload).encode("utf-8")


def _required_payload_text(payload: cabc.Mapping[str, object], name: str) -> str:
    """Return one required bounded Check Run payload string."""
    value = payload.get(name)
    if not isinstance(value, str) or not value:
        raise CheckRunPublicationError.missing_payload_text(name)
    return value


def _response_mapping(response: bytes) -> cabc.Mapping[str, object]:
    """Parse the bounded JSON response required for idempotency selection."""
    try:
        decoded = json.loads(response)
    except json.JSONDecodeError as error:
        raise CheckRunPublicationError.response_is_not_json() from error
    if not isinstance(decoded, dict):
        raise CheckRunPublicationError.malformed_response()
    return decoded


def _check_run_entries(
    response_body: cabc.Mapping[str, object],
) -> list[cabc.Mapping[str, object]]:
    """Return the mapping entries of one bounded ``check_runs`` response list."""
    check_runs = response_body.get("check_runs")
    if not isinstance(check_runs, list):
        raise CheckRunPublicationError.malformed_response()
    return [entry for entry in check_runs if isinstance(entry, dict)]


def _matches_check_run(check_run: object, external_id: str) -> bool:
    """Return whether one response member is this publisher's bounded run."""
    return (
        isinstance(check_run, dict)
        and check_run.get("name") == CHECK_RUN_NAME
        and check_run.get("external_id") == external_id
    )


def _matching_id_in_page(
    response_body: cabc.Mapping[str, object], external_id: str
) -> int | None:
    """Return the ID of one lookup page's matching Check Run, or ``None``."""
    return next(
        (
            check_run_id
            for check_run in _check_run_entries(response_body)
            if _matches_check_run(check_run, external_id)
            if (check_run_id := check_run.get("id")) is not None
            if isinstance(check_run_id, int)
        ),
        None,
    )


def _has_next_page(response_body: cabc.Mapping[str, object], page_number: int) -> bool:
    """Return whether the server's bounded total holds a page after this one.

    Parameters
    ----------
    response_body
        Decoded lookup response carrying the server's ``total_count``.
    page_number
        One-based number of the page just read.

    Returns
    -------
    bool
        Whether the server reports further Check Runs beyond ``page_number``.

    Raises
    ------
    malformed_response
        With the malformed-response diagnostic when the required
        ``total_count`` is absent, boolean, or not an integer.
    lookup_exceeds_limit
        With the over-limit diagnostic when the total exceeds the Check Runs
        these pages cover. ``MAXIMUM_CHECK_RUN_PAGES`` bounds the lookup at
        1020 runs, just past the endpoint's documented ceiling, so a lookup
        that cannot be exhaustive never publishes a possible duplicate.
    """
    total_count = response_body.get("total_count")
    if isinstance(total_count, bool) or not isinstance(total_count, int):
        raise CheckRunPublicationError.malformed_response()
    if not 0 <= total_count <= MAXIMUM_CHECK_RUN_PAGES * CHECK_RUN_PAGE_SIZE:
        raise CheckRunPublicationError.lookup_exceeds_limit()
    return total_count > page_number * CHECK_RUN_PAGE_SIZE
