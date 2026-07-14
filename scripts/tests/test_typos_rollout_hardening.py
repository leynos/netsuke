"""Focused regressions for hardened shared spelling-policy refreshes."""

from __future__ import annotations

import importlib
import json
import types
import urllib.error
import urllib.request
from pathlib import Path

import pytest

from typos_rollout_test_support import dictionary_text as _dictionary_text

SCRIPT_DIRECTORY = Path(__file__).resolve().parents[1]


class ValidResponse:
    """Provide configurable valid dictionary bytes at the HTTP boundary."""

    status = 200

    def __init__(
        self,
        *,
        stem: str = "organ",
        headers: dict[str, str] | None = None,
    ) -> None:
        self._stem = stem
        self.headers = {} if headers is None else headers

    def read(self) -> bytes:
        """Return valid shared dictionary bytes."""
        return _dictionary_text(self._stem).encode()

    def __enter__(self) -> ValidResponse:
        """Enter the fake response context."""
        return self

    def __exit__(self, *_args: object) -> None:
        """Leave the fake response context."""


@pytest.fixture(name="rollout_modules")
def rollout_modules_fixture(
    monkeypatch: pytest.MonkeyPatch,
) -> tuple[
    types.ModuleType,
    types.ModuleType,
    types.ModuleType,
    types.ModuleType,
]:
    """Import helpers through the top-level paths used by the generator."""
    monkeypatch.syspath_prepend(str(SCRIPT_DIRECTORY))
    names = (
        "typos_rollout_cache",
        "typos_rollout_http",
        "typos_rollout",
        "generate_typos_config",
    )
    importlib.invalidate_caches()
    cache, refresh, rollout, generator = (
        importlib.import_module(name) for name in names
    )
    return cache, refresh, rollout, generator


def test_oxford_adverb_suffix_is_generated(
    rollout_modules: tuple[
        types.ModuleType,
        types.ModuleType,
        types.ModuleType,
        types.ModuleType,
    ],
) -> None:
    """Oxford adverbs are accepted and plain-British forms are corrected."""
    _, _, rollout, _ = rollout_modules
    mappings = rollout.generate_word_mappings(rollout.Dictionary(stems=("recogn",)))

    assert mappings["recognizably"] == "recognizably"
    assert mappings["recognisably"] == "recognizably"


def test_changed_etag_overrides_unchanged_date(
    rollout_modules: tuple[
        types.ModuleType,
        types.ModuleType,
        types.ModuleType,
        types.ModuleType,
    ],
    tmp_path: Path,
) -> None:
    """A changed ETag refreshes even when Last-Modified is unchanged."""
    _, _, rollout, _ = rollout_modules
    cache = tmp_path / "cache.toml"
    metadata = tmp_path / "cache.json"
    source = "https://example.test/base.toml"
    modified = "Fri, 10 Jul 2026 08:00:00 GMT"
    cache.write_text(_dictionary_text("original"), encoding="utf-8")
    metadata.write_text(
        json.dumps(
            {
                "etag": '"estate-v1"',
                "last_modified": modified,
                "source": source,
            }
        ),
        encoding="utf-8",
    )

    result = rollout.refresh_base(
        source,
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
            opener=lambda *_args, **_kwargs: ValidResponse(
                stem="replacement",
                headers={"ETag": '"estate-v2"', "Last-Modified": modified},
            ),
        ),
    )

    assert result.status == "refreshed"
    assert rollout.load_dictionary(cache).stems == ("replacement",)


@pytest.mark.parametrize(
    "target",
    ["http://example.test/base.toml", "ftp://example.test/base.toml"],
)
def test_redirect_target_requires_https(
    rollout_modules: tuple[
        types.ModuleType,
        types.ModuleType,
        types.ModuleType,
        types.ModuleType,
    ],
    target: str,
) -> None:
    """Redirects cannot downgrade or leave HTTPS transport."""
    _, refresh, _, _ = rollout_modules
    handler = refresh._HttpsRedirectHandler()

    with pytest.raises(
        refresh.InsecureSourceError,
        match="redirect must use HTTPS",
    ):
        handler.redirect_request(
            urllib.request.Request("https://example.test/base.toml"),
            None,
            302,
            "Found",
            {},
            target,
        )


def test_default_refresh_uses_guarded_https_opener(
    rollout_modules: tuple[
        types.ModuleType,
        types.ModuleType,
        types.ModuleType,
        types.ModuleType,
    ],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """Production refresh delegates through the guarded redirect opener."""
    _, refresh, rollout, _ = rollout_modules
    requests: list[object] = []

    class GuardedOpener:
        """Capture calls through the configured HTTPS-only opener."""

        def open(self, request: object, *, timeout: float) -> ValidResponse:
            """Return a valid response after recording the guarded call."""
            assert timeout == pytest.approx(30.0)
            requests.append(request)
            return ValidResponse()

    monkeypatch.setattr(refresh, "_HTTPS_OPENER", GuardedOpener())

    result = rollout.refresh_base(
        "https://example.test/base.toml",
        tmp_path / "cache.toml",
        rollout.RefreshOptions(
            metadata=tmp_path / "cache.json",
        ),
    )

    assert result.status == "refreshed"
    assert len(requests) == 1


def test_http_status_and_persistence_errors_propagate(
    rollout_modules: tuple[
        types.ModuleType,
        types.ModuleType,
        types.ModuleType,
        types.ModuleType,
    ],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """HTTP status and local write failures never become stale successes."""
    cache_module, _, rollout, _ = rollout_modules
    cache = tmp_path / "cache.toml"
    metadata = tmp_path / "cache.json"
    cache.write_text(_dictionary_text(), encoding="utf-8")
    not_found = urllib.error.HTTPError(
        "https://example.test/missing",
        404,
        "Not Found",
        hdrs=None,
        fp=None,
    )

    def missing(*_args: object, **_kwargs: object) -> ValidResponse:
        """Raise the authority's HTTP response."""
        raise not_found

    with pytest.raises(urllib.error.HTTPError) as raised:
        rollout.refresh_base(
            "https://example.test/missing",
            cache,
            rollout.RefreshOptions(
                metadata=metadata,
                opener=missing,
            ),
        )
    assert raised.value is not_found

    def denied(*_args: object, **_kwargs: object) -> None:
        """Model denied persistence at the atomic-write boundary."""
        raise PermissionError("cache is read-only")

    monkeypatch.setattr(cache_module, "atomic_write", denied)
    with pytest.raises(PermissionError, match="cache is read-only"):
        rollout.refresh_base(
            "https://example.test/base",
            cache,
            rollout.RefreshOptions(
                metadata=metadata,
                opener=lambda *_args, **_kwargs: ValidResponse(stem="replacement"),
            ),
        )


@pytest.mark.parametrize(
    "error",
    [
        pytest.param(
            urllib.error.HTTPError(
                "https://example.test/missing",
                404,
                "Not Found",
                hdrs=None,
                fp=None,
            ),
            id="http-status",
        ),
        pytest.param(PermissionError("cache is read-only"), id="persistence"),
    ],
)
def test_generator_propagates_non_connectivity_failures(
    rollout_modules: tuple[
        types.ModuleType,
        types.ModuleType,
        types.ModuleType,
        types.ModuleType,
    ],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    error: OSError,
) -> None:
    """Generator fallback is limited to connectivity-domain failures."""
    _, _, rollout, generator = rollout_modules
    (tmp_path / "typos.toml").write_text(
        '[default]\nlocale = "en-gb"\n',
        encoding="utf-8",
    )

    def fail(*_args: object, **_kwargs: object) -> None:
        """Raise the selected non-connectivity failure."""
        raise error

    monkeypatch.setattr(rollout, "refresh_base", fail)
    with pytest.raises(type(error)) as raised:
        generator.main(
            repository=tmp_path,
            source="https://example.test/base",
        )

    assert raised.value is error


@pytest.mark.parametrize("failure_stage", ["write", "close", "replace"])
def test_atomic_write_cleans_temporary_file_after_failure(
    rollout_modules: tuple[
        types.ModuleType,
        types.ModuleType,
        types.ModuleType,
        types.ModuleType,
    ],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    failure_stage: str,
) -> None:
    """Write, close, and replacement failures remove the temporary file."""
    cache_module, _, _, _ = rollout_modules
    temporary = tmp_path / ".typos.toml.failure"
    temporary.touch()

    class FailingStream:
        """Model a named temporary stream with one selected failure."""

        name = str(temporary)

        def __enter__(self) -> FailingStream:
            """Enter the fake stream context."""
            return self

        def write(self, content: bytes) -> None:
            """Write bytes unless this case models a write failure."""
            if failure_stage == "write":
                raise OSError("write failure")
            temporary.write_bytes(content)

        def __exit__(self, *_args: object) -> None:
            """Close unless this case models a close failure."""
            if failure_stage == "close":
                raise OSError("close failure")

    monkeypatch.setattr(
        cache_module.tempfile,
        "NamedTemporaryFile",
        lambda **_kwargs: FailingStream(),
    )
    if failure_stage == "replace":

        def fail_replace(_path: Path, _destination: Path) -> None:
            """Model an atomic replacement failure."""
            raise OSError("replace failure")

        monkeypatch.setattr(cache_module.pathlib.Path, "replace", fail_replace)

    with pytest.raises(OSError, match=f"{failure_stage} failure"):
        cache_module.atomic_write(tmp_path / "typos.toml", b"content")

    assert not temporary.exists()
