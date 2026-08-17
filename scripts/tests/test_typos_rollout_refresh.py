"""Refresh, transport-security, and cache spelling-policy tests."""

from __future__ import annotations

import email.message
import json
import os
import tomllib
import typing as typ
import urllib.error
import urllib.request
from pathlib import Path

import pytest

from typos_rollout_test_support import dictionary_text as _dictionary_text

if typ.TYPE_CHECKING:
    import types


def test_local_refresh_keeps_a_newer_cache(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """An older local authority cannot replace a newer untracked cache."""
    _, rollout, _ = rollout_modules
    source = tmp_path / "shared.toml"
    cache = tmp_path / ".typos-base.toml"
    metadata = tmp_path / ".typos-base.json"
    source.write_text(_dictionary_text(), encoding="utf-8")
    source.touch()
    rollout.refresh_base(
        source,
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
        ),
    )
    cache.write_text(_dictionary_text("newer"), encoding="utf-8")
    cache.touch()
    source_mtime = source.stat().st_mtime_ns
    cache_mtime = max(cache.stat().st_mtime_ns, source_mtime + 1)
    os.utime(cache, ns=(cache_mtime, cache_mtime))

    result = rollout.refresh_base(
        source,
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
        ),
    )

    assert result.status == "current"
    assert rollout.load_dictionary(cache).stems == ("newer",)


def test_offline_refresh_requires_and_reuses_valid_cache(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """Offline mode fails closed before reusing a validated cache."""
    _, rollout, _ = rollout_modules
    cache = tmp_path / "base.toml"
    metadata = tmp_path / "base.json"

    with pytest.raises(FileNotFoundError, match="no cached shared dictionary"):
        rollout.refresh_base(
            "https://example.invalid/base",
            cache,
            rollout.RefreshOptions(
                metadata=metadata,
                offline=True,
            ),
        )

    cache.write_text(_dictionary_text(), encoding="utf-8")
    result = rollout.refresh_base(
        "https://example.invalid/base",
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
            offline=True,
        ),
    )

    assert result.status == "offline-cache"


def test_local_refresh_switches_authority_and_records_metadata(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """A different explicit authority replaces a cache regardless of mtime."""
    _, rollout, _ = rollout_modules
    first = tmp_path / "first.toml"
    second = tmp_path / "second.toml"
    cache = tmp_path / "cache.toml"
    metadata = tmp_path / "cache.json"
    first.write_text(_dictionary_text("first"), encoding="utf-8")
    second.write_text(_dictionary_text("second"), encoding="utf-8")
    os.utime(first, ns=(3_000_000_000, 3_000_000_000))
    os.utime(second, ns=(1_000_000_000, 1_000_000_000))
    rollout.refresh_base(
        first,
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
        ),
    )

    result = rollout.refresh_base(
        second,
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
        ),
    )

    assert result.status == "refreshed"
    assert rollout.load_dictionary(cache).stems == ("second",)
    assert json.loads(metadata.read_text(encoding="utf-8"))["source"] == str(
        second.resolve()
    )


def test_http_refresh_scopes_validators_and_preserves_newer_cache(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """Remote refresh reuses validators only for their original source."""
    _, rollout, _ = rollout_modules
    cache = tmp_path / "cache.toml"
    metadata = tmp_path / "cache.json"
    requests: list[urllib.request.Request] = []

    class Response:
        """Provide the HTTP response surface consumed by the helper."""

        status = 200
        headers: typ.ClassVar[dict[str, str]] = {
            "ETag": '"estate-v1"',
            "Last-Modified": "Fri, 10 Jul 2026 08:00:00 GMT",
        }

        def read(self) -> bytes:
            """Return a valid shared dictionary."""
            return _dictionary_text().encode()

        def __enter__(self) -> Response:
            """Enter the fake response context."""
            return self

        def __exit__(self, *_args: object) -> None:
            """Leave the fake response context."""

    def open_response(request: urllib.request.Request, *, timeout: float) -> Response:
        """Capture the request passed to the network boundary."""
        assert timeout == pytest.approx(30.0)
        requests.append(request)
        return Response()

    monkeypatch.setattr(rollout.urllib.request, "urlopen", open_response)

    first = rollout.refresh_base(
        "https://example.test/base.toml",
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
        ),
    )
    second = rollout.refresh_base(
        "https://example.test/base.toml",
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
        ),
    )
    replacement = rollout.refresh_base(
        "https://example.test/replacement.toml",
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
        ),
    )

    assert first.status == "refreshed"
    assert second.status == "current"
    assert requests[1].get_header("If-none-match") == '"estate-v1"'
    assert replacement.status == "refreshed"
    assert requests[2].get_header("If-none-match") is None
    assert requests[2].get_header("If-modified-since") is None


def test_remote_failure_reuses_only_a_valid_stale_cache(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """Network failure keeps validated data and propagates without it."""
    _, rollout, _ = rollout_modules
    cache = tmp_path / "cache.toml"
    metadata = tmp_path / "cache.json"

    def fail(*_args: object, **_kwargs: object) -> None:
        """Model an unavailable remote authority."""
        message = "offline"
        raise urllib.error.URLError(message)

    monkeypatch.setattr(rollout.urllib.request, "urlopen", fail)

    with pytest.raises(rollout.NetworkUnavailableError):
        rollout.refresh_base(
            "https://example.test/base",
            cache,
            rollout.RefreshOptions(
                metadata=metadata,
            ),
        )

    cache.write_text(_dictionary_text(), encoding="utf-8")
    result = rollout.refresh_base(
        "https://example.test/base",
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
        ),
    )

    assert result.status == "stale-cache"


def test_remote_refresh_rejects_insecure_and_invalid_content(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """The remote boundary requires HTTPS and validates bytes before install."""
    _, rollout, _ = rollout_modules
    cache = tmp_path / "cache.toml"
    metadata = tmp_path / "cache.json"

    with pytest.raises(ValueError, match="must use HTTPS"):
        rollout.refresh_base(
            "http://example.test/base",
            cache,
            rollout.RefreshOptions(
                metadata=metadata,
            ),
        )

    class InvalidResponse:
        """Return malformed TOML from an otherwise successful response."""

        status = 200
        headers: typ.ClassVar[dict[str, str]] = {}

        def read(self) -> bytes:
            """Return malformed bytes."""
            return b"not = [valid"

        def __enter__(self) -> InvalidResponse:
            """Enter the fake response context."""
            return self

        def __exit__(self, *_args: object) -> None:
            """Leave the fake response context."""

    monkeypatch.setattr(
        rollout.urllib.request, "urlopen", lambda *_args, **_kwargs: InvalidResponse()
    )

    with pytest.raises(tomllib.TOMLDecodeError):
        rollout.refresh_base(
            "https://example.test/base",
            cache,
            rollout.RefreshOptions(
                metadata=metadata,
            ),
        )
    assert not cache.exists()


def test_metadata_reader_handles_invalid_and_non_object_json(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """Malformed or non-object freshness metadata is safely ignored."""
    _, rollout, _ = rollout_modules
    metadata = tmp_path / "cache.json"

    metadata.write_text("not-json", encoding="utf-8")
    assert rollout._read_metadata(metadata) == {}
    metadata.write_text("[]", encoding="utf-8")
    assert rollout._read_metadata(metadata) == {}


def test_http_error_translation_handles_not_modified_and_stale_cache(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """HTTP status handling distinguishes current, stale and absent data."""
    _, rollout, _ = rollout_modules
    cache = tmp_path / "cache.toml"
    cache.write_text(_dictionary_text(), encoding="utf-8")
    headers = email.message.Message()
    not_modified = urllib.error.HTTPError(
        "https://example.test/base", 304, "not modified", headers, None
    )
    unavailable = urllib.error.HTTPError(
        "https://example.test/base", 503, "unavailable", headers, None
    )

    assert rollout._http_error_result(cache, not_modified).status == "current"
    with pytest.raises(urllib.error.HTTPError):
        rollout._http_error_result(cache, unavailable)
    cache.unlink()
    with pytest.raises(urllib.error.HTTPError):
        rollout._http_error_result(cache, unavailable)


def test_remote_freshness_uses_dates_and_falls_back_on_invalid_values(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
) -> None:
    """Last-Modified comparison remains conservative for malformed dates."""
    _, rollout, _ = rollout_modules

    assert rollout._remote_is_not_newer(
        {"last_modified": "Fri, 10 Jul 2026 08:00:00 GMT"},
        {"Last-Modified": "Fri, 10 Jul 2026 07:00:00 GMT"},
    )
    assert rollout._remote_is_not_newer(
        {"last_modified": "invalid"}, {"Last-Modified": "invalid"}
    )
    assert not rollout._remote_is_not_newer({}, {"Last-Modified": "invalid"})
