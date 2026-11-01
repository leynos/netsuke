"""Build crates.io requests behind an HTTPS host allowlist.

The request handler validates every outgoing request. urllib sends each
redirect target through the same handler before opening it, so redirected
responses cannot bypass the fixed registry hosts.

>>> is_registry_url("https://crates.io/api/v1")
True
>>> is_registry_url("https://example.invalid/api/v1")
False
"""

import typing as typ
import urllib.parse
import urllib.request

ALLOWED_HOSTS = frozenset({"crates.io", "static.crates.io"})


class HttpResponse(typ.Protocol):
    """Describe response methods consumed by the transport CLI."""

    status: int

    def read(self) -> bytes:
        """Return the complete response body."""

    def close(self) -> None:
        """Close the response after reading its body."""


class RegistryUrlError(OSError):
    """Report a request URL outside the fixed HTTPS host allowlist."""

    def __str__(self) -> str:
        """Explain the fixed registry transport boundary."""
        return "crates.io transport URL is outside the HTTPS host allowlist"


def _is_allowed_url(url: str, allowed_hosts: frozenset[str]) -> bool:
    """Check the HTTPS scheme and host against one request handler's policy."""
    parsed = urllib.parse.urlsplit(url)
    return (
        parsed.scheme == "https"
        and parsed.hostname in allowed_hosts
        and parsed.username is None
        and parsed.password is None
        and parsed.port is None
        and not parsed.fragment
    )


def is_registry_url(url: str) -> bool:
    """Allow only HTTPS URLs on crates.io's two public hosts."""
    return _is_allowed_url(url, ALLOWED_HOSTS)


def build_request(
    url: str,
    *,
    headers: dict[str, str],
    data: bytes | None = None,
    method: str | None = None,
) -> urllib.request.Request:
    """Build an HTTP request only for an allowlisted registry URL."""
    if not is_registry_url(url):
        raise RegistryUrlError
    # S310 cannot infer the fixed HTTPS host allowlist checked above.
    return urllib.request.Request(  # ruff: ignore[suspicious-url-open-usage]
        url, data=data, headers=dict(headers), method=method
    )


class _AllowlistedRequestHandler(urllib.request.BaseHandler):
    """Check every outgoing request, including urllib redirect targets."""

    def __init__(self, allowed_hosts: frozenset[str]) -> None:
        """Store the HTTPS hosts this opener may contact."""
        self._allowed_hosts = allowed_hosts

    def _check_request(self, request: urllib.request.Request) -> urllib.request.Request:
        """Reject an outgoing request unless its URL passes this handler's policy."""
        if not _is_allowed_url(request.full_url, self._allowed_hosts):
            raise RegistryUrlError
        return request

    http_request = _check_request
    https_request = _check_request
    ftp_request = _check_request
    file_request = _check_request


_HTTP_OPENER = urllib.request.build_opener(_AllowlistedRequestHandler(ALLOWED_HOSTS))


def default_opener(request: urllib.request.Request, *, timeout: float) -> HttpResponse:
    """Open an allowlisted request with redirect targets checked at each hop."""
    if not is_registry_url(request.full_url):
        raise RegistryUrlError
    return _HTTP_OPENER.open(request, timeout=timeout)
