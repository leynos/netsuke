"""Refresh the shared spelling dictionary across local and HTTPS authorities.

This module owns cache freshness, transport security, and persistence
coordination. The renderer remains in :mod:`typos_rollout`; callers outside
the spelling helper should not reuse these infrastructure internals.
"""

from __future__ import annotations

import email.utils
import json
import pathlib
import urllib.error
import urllib.parse
import urllib.request
from collections import abc as cabc

import typos_rollout_cache

ContentValidator = cabc.Callable[[bytes], None]
Opener = cabc.Callable[..., typos_rollout_cache.RemoteResponse]
HTTP_NOT_MODIFIED = 304


class NetworkUnavailableError(OSError):
    """Report that the remote dictionary authority could not be reached."""


class InsecureSourceError(ValueError):
    """Report a dictionary source or redirect that does not use HTTPS."""


def _read_metadata(path: pathlib.Path) -> dict[str, object]:
    """Read best-effort HTTP freshness metadata."""
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (FileNotFoundError, json.JSONDecodeError):
        return {}
    return value if isinstance(value, dict) else {}


def _write_metadata(
    path: pathlib.Path,
    metadata: cabc.Mapping[str, object],
) -> None:
    """Atomically write HTTP freshness metadata."""
    content = (json.dumps(metadata, sort_keys=True) + "\n").encode()
    typos_rollout_cache.atomic_write(path, content)


def _valid_cache(cache: pathlib.Path, validate: ContentValidator) -> bool:
    """Return whether *cache* contains a valid shared dictionary."""
    try:
        validate(cache.read_bytes())
    except (
        FileNotFoundError,
        OSError,
        TypeError,
        ValueError,
    ):
        return False
    return True


def _remote_is_not_newer(
    saved: cabc.Mapping[str, object],
    headers: cabc.Mapping[str, str],
) -> bool:
    """Return whether HTTP validators prove the response is not newer."""
    etag = headers.get("ETag")
    saved_etag = saved.get("etag")
    if isinstance(etag, str) and isinstance(saved_etag, str):
        return etag == saved_etag
    modified = headers.get("Last-Modified")
    saved_modified = saved.get("last_modified")
    if not isinstance(modified, str) or not isinstance(saved_modified, str):
        return False
    try:
        return email.utils.parsedate_to_datetime(
            modified
        ) <= email.utils.parsedate_to_datetime(saved_modified)
    except (TypeError, ValueError):
        return modified == saved_modified


def _local_cache_is_current(
    cache: pathlib.Path,
    saved: cabc.Mapping[str, object],
    source_name: str,
    source_mtime_ns: int,
    validate: ContentValidator,
) -> bool:
    """Return whether metadata proves a valid local-source cache is current."""
    saved_mtime = saved.get("mtime_ns")
    has_matching_source = saved.get("source") == source_name
    has_new_enough_mtime = (
        isinstance(saved_mtime, int) and source_mtime_ns <= saved_mtime
    )
    return (
        _valid_cache(cache, validate) and has_matching_source and has_new_enough_mtime
    )


def _refresh_local(
    source: pathlib.Path,
    cache: pathlib.Path,
    metadata: pathlib.Path,
    validate: ContentValidator,
) -> typos_rollout_cache.RefreshResult:
    """Refresh from a local authoritative copy when it is newer."""
    source_stat = source.stat()
    source_name = str(source.resolve())
    saved = _read_metadata(metadata)
    if _local_cache_is_current(
        cache,
        saved,
        source_name,
        source_stat.st_mtime_ns,
        validate,
    ):
        return typos_rollout_cache.RefreshResult("current", cache)
    content = source.read_bytes()
    validate(content)
    typos_rollout_cache.atomic_write(cache, content)
    _write_metadata(
        metadata,
        {"source": source_name, "mtime_ns": source_stat.st_mtime_ns},
    )
    return typos_rollout_cache.RefreshResult("refreshed", cache)


def _conditional_headers(saved: cabc.Mapping[str, object]) -> dict[str, str]:
    """Build conditional HTTP headers from persisted validators."""
    headers: dict[str, str] = {}
    etag = saved.get("etag")
    if isinstance(etag, str):
        headers["If-None-Match"] = etag
    last_modified = saved.get("last_modified")
    if isinstance(last_modified, str):
        headers["If-Modified-Since"] = last_modified
    return headers


class _HttpsRedirectHandler(urllib.request.HTTPRedirectHandler):
    """Reject redirects that leave the HTTPS transport boundary."""

    def redirect_request(
        self,
        request: urllib.request.Request,
        file_pointer: object,
        code: int,
        message: str,
        headers: object,
        new_url: str,
    ) -> urllib.request.Request | None:
        """Follow only redirects whose resolved target remains HTTPS."""
        if urllib.parse.urlsplit(new_url).scheme != "https":
            error_message = f"shared dictionary redirect must use HTTPS: {new_url}"
            raise InsecureSourceError(error_message)
        return super().redirect_request(
            request,
            file_pointer,
            code,
            message,
            headers,
            new_url,
        )


_HTTPS_OPENER = urllib.request.build_opener(_HttpsRedirectHandler())


def _https_request(
    source: str,
    headers: cabc.Mapping[str, str],
) -> urllib.request.Request:
    """Build a request after constraining the shared source to HTTPS."""
    if urllib.parse.urlsplit(source).scheme != "https":
        message = f"shared dictionary URL must use HTTPS: {source}"
        raise InsecureSourceError(message)
    return urllib.request.Request(source, headers=dict(headers))


def _write_remote_cache(
    source: str,
    targets: typos_rollout_cache.CacheTargets,
    content: bytes,
    headers: cabc.Mapping[str, str],
    validate: ContentValidator,
) -> typos_rollout_cache.RefreshResult:
    """Validate and atomically persist an HTTP dictionary response."""
    validate(content)
    typos_rollout_cache.atomic_write(targets.cache, content)
    _write_metadata(
        targets.metadata,
        {
            "source": source,
            "etag": headers.get("ETag"),
            "last_modified": headers.get("Last-Modified"),
        },
    )
    return typos_rollout_cache.RefreshResult("refreshed", targets.cache)


def _remote_response_result(
    source: str,
    targets: typos_rollout_cache.CacheTargets,
    saved: cabc.Mapping[str, object],
    response: typos_rollout_cache.RemoteResponse,
    validate: ContentValidator,
) -> typos_rollout_cache.RefreshResult:
    """Return the cache result for a successful HTTP response."""
    if response.status == HTTP_NOT_MODIFIED and _valid_cache(targets.cache, validate):
        return typos_rollout_cache.RefreshResult("current", targets.cache)
    if _valid_cache(targets.cache, validate) and _remote_is_not_newer(
        saved, response.headers
    ):
        return typos_rollout_cache.RefreshResult("current", targets.cache)
    try:
        content = response.read()
    except urllib.error.URLError as error:
        message = f"shared dictionary authority is unavailable: {source}"
        raise NetworkUnavailableError(message) from error
    return _write_remote_cache(
        source,
        targets,
        content,
        response.headers,
        validate,
    )


def _stale_cache_or_raise(
    cache: pathlib.Path,
    error: NetworkUnavailableError,
    validate: ContentValidator,
) -> typos_rollout_cache.RefreshResult:
    """Return a valid stale cache or propagate the connectivity failure."""
    if _valid_cache(cache, validate):
        return typos_rollout_cache.RefreshResult("stale-cache", cache)
    raise error


def _http_error_result(
    cache: pathlib.Path,
    error: urllib.error.HTTPError,
    validate: ContentValidator,
) -> typos_rollout_cache.RefreshResult:
    """Translate an HTTP response into a current result or propagate it."""
    if error.code == HTTP_NOT_MODIFIED and _valid_cache(cache, validate):
        return typos_rollout_cache.RefreshResult("current", cache)
    raise error


def _refresh_http(
    source: str,
    cache: pathlib.Path,
    metadata: pathlib.Path,
    validate: ContentValidator,
    opener: Opener | None,
) -> typos_rollout_cache.RefreshResult:
    """Refresh a cache from a validated HTTPS source with stale fallback."""
    saved = _read_metadata(metadata)
    if saved.get("source") != source:
        saved = {}
    request = _https_request(source, _conditional_headers(saved))
    open_remote = _HTTPS_OPENER.open if opener is None else opener
    try:
        response_context = open_remote(request, timeout=30.0)
    except urllib.error.HTTPError as error:
        return _http_error_result(cache, error, validate)
    except urllib.error.URLError:
        message = f"shared dictionary authority is unavailable: {source}"
        unavailable = NetworkUnavailableError(message)
        return _stale_cache_or_raise(cache, unavailable, validate)
    with response_context as response:
        try:
            return _remote_response_result(
                source,
                typos_rollout_cache.CacheTargets(cache, metadata),
                saved,
                response,
                validate,
            )
        except NetworkUnavailableError as error:
            return _stale_cache_or_raise(cache, error, validate)


def refresh_base(
    source: str | pathlib.Path,
    cache: pathlib.Path,
    *,
    metadata: pathlib.Path,
    validate: ContentValidator,
    offline: bool = False,
    opener: Opener | None = None,
) -> typos_rollout_cache.RefreshResult:
    """Refresh an untracked cache when its authoritative copy is newer.

    Parameters
    ----------
    source
        Local path or HTTPS URL for the authoritative shared dictionary.
    cache
        Untracked local cache destination.
    metadata
        Sidecar path containing source identity and freshness validators.
    validate
        Callback that rejects invalid dictionary bytes.
    offline
        Reuse a valid cache without contacting the authority.
    opener
        Injectable HTTPS opener. The guarded production opener is the default.

    Returns
    -------
    typos_rollout_cache.RefreshResult
        Refresh status and validated cache path.

    Raises
    ------
    FileNotFoundError
        If offline mode has no valid cache.
    NetworkUnavailableError
        If the authority is unreachable and no valid stale cache exists.
    InsecureSourceError
        If an authority or redirect does not use HTTPS.
    OSError, urllib.error.HTTPError
        If local persistence fails or the authority returns an HTTP error.
    TypeError, ValueError
        If dictionary validation fails.
    """
    if offline:
        if not _valid_cache(cache, validate):
            message = f"no cached shared dictionary at {cache}"
            raise FileNotFoundError(message)
        return typos_rollout_cache.RefreshResult("offline-cache", cache)
    if isinstance(source, pathlib.Path) or "://" not in str(source):
        return _refresh_local(pathlib.Path(source), cache, metadata, validate)
    return _refresh_http(str(source), cache, metadata, validate, opener)
