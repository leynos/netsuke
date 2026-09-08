"""Publish idempotent, bounded GitHub Check Runs for trusted workflow helpers.

Only ``coverage_pr_submission.py`` may construct this adapter in production.
It owns GitHub's fixed API boundary; callers provide already-bounded payload
data and tests may substitute a narrow publisher callable at that boundary.
"""

import collections.abc as cabc
import json
import typing as typ
import urllib.parse
import urllib.request

type CheckRunPublisher = cabc.Callable[[dict[str, object]], None]

CHECK_RUN_NAME = "CodeScene coverage"
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


class GitHubCheckRunPublisher:
    """Publish one Check Run idempotently through GitHub's REST API.

    Parameters
    ----------
    repository
        Trusted owner-and-repository identifier supplied by GitHub Actions.
    token
        Step-local GitHub token used only in API request headers.
    api_origin
        Fixed GitHub API origin in production. Tests may supply a local HTTP
        server to exercise the protocol boundary without GitHub credentials.

    Attributes
    ----------
    repository
        Trusted repository whose Check Runs this adapter publishes.

    Notes
    -----
    The originating workflow run ID is the idempotency key. The adapter first
    finds an existing matching Check Run and updates it; otherwise it creates
    one. Workflow-level concurrency serializes the same key before this API
    protocol runs.
    """

    def __init__(
        self,
        repository: str,
        token: str,
        api_origin: str = GITHUB_API_ORIGIN,
    ) -> None:
        """Store the trusted API boundary used by subsequent publications."""
        self.repository = repository
        self._token = token
        self._api_origin = api_origin.rstrip("/")

    def __call__(self, payload: dict[str, object]) -> None:
        """Create or update the Check Run selected by its bounded identity.

        Parameters
        ----------
        payload
            Completed Check Run payload containing the trusted commit SHA and
            originating workflow run ID.

        """
        check_run_id = self._existing_check_run_id(payload)
        endpoint = self._check_run_endpoint(check_run_id)
        method = "PATCH" if check_run_id is not None else "POST"
        self._request(method, endpoint, payload)

    def _check_run_endpoint(self, check_run_id: int | None = None) -> str:
        """Return the fixed Check Runs endpoint, optionally for one update."""
        endpoint = f"{self._api_origin}/repos/{self.repository}/check-runs"
        return endpoint if check_run_id is None else f"{endpoint}/{check_run_id}"

    def _existing_check_run_id(self, payload: cabc.Mapping[str, object]) -> int | None:
        """Return the existing run ID selected by commit, name, and external ID."""
        commit_sha = _required_payload_text(payload, "head_sha")
        external_id = _required_payload_text(payload, "external_id")
        encoded_name = urllib.parse.urlencode({"check_name": CHECK_RUN_NAME})
        endpoint = (
            f"{self._api_origin}/repos/{self.repository}/commits/{commit_sha}"
            f"/check-runs?{encoded_name}"
        )
        response = self._request("GET", endpoint)
        response_body = _response_mapping(response)
        check_runs = response_body.get("check_runs")
        if not isinstance(check_runs, list):
            raise CheckRunPublicationError.malformed_response()
        return next(
            (
                check_run_id
                for check_run in check_runs
                if _matches_check_run(check_run, external_id)
                if (check_run_id := check_run.get("id")) is not None
                if isinstance(check_run_id, int)
            ),
            None,
        )

    def _request(
        self,
        method: str,
        endpoint: str,
        payload: cabc.Mapping[str, object] | None = None,
    ) -> bytes:
        """Send one bounded GitHub API request and read its limited response."""
        data = None if payload is None else json.dumps(payload).encode("utf-8")
        # ruff:ignore[suspicious-url-open-usage] -- the endpoint is the fixed GitHub API host; tests inject only a local contract server.
        request = urllib.request.Request(
            endpoint,
            data=data,
            headers={
                "Accept": "application/vnd.github+json",
                "Authorization": f"Bearer {self._token}",
                "X-GitHub-Api-Version": GITHUB_API_VERSION,
            },
            method=method,
        )
        # ruff:ignore[suspicious-url-open-usage] -- the endpoint is the fixed GitHub API host; tests inject only a local contract server.
        with urllib.request.urlopen(request, timeout=30) as response:
            response_bytes = response.read(MAXIMUM_RESPONSE_BYTES + 1)
        if len(response_bytes) > MAXIMUM_RESPONSE_BYTES:
            raise CheckRunPublicationError.response_exceeds_limit()
        return response_bytes


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


def _matches_check_run(check_run: object, external_id: str) -> bool:
    """Return whether one response member is this publisher's bounded run."""
    return (
        isinstance(check_run, dict)
        and check_run.get("name") == CHECK_RUN_NAME
        and check_run.get("external_id") == external_id
    )
