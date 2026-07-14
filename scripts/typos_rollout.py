"""Refresh and render shared en-GB-oxendict ``typos`` configuration."""

from __future__ import annotations

import dataclasses as dc
import json
import pathlib
import tomllib
import typing as typ
import urllib.error
import urllib.parse
import urllib.request

import typos_rollout_cache
import typos_rollout_http

if typ.TYPE_CHECKING:
    import collections.abc as cabc

RefreshResult = typos_rollout_cache.RefreshResult
RefreshOptions = typos_rollout_http.RefreshOptions
_CacheTargets = typos_rollout_cache.CacheTargets
_RemoteResponse = typos_rollout_cache.RemoteResponse
NetworkUnavailableError = typos_rollout_http.NetworkUnavailableError
InsecureSourceError = typos_rollout_http.InsecureSourceError
_HttpsRedirectHandler = typos_rollout_http._HttpsRedirectHandler
_HTTPS_OPENER = typos_rollout_http._HTTPS_OPENER
_read_metadata = typos_rollout_http._read_metadata
_remote_is_not_newer = typos_rollout_http._remote_is_not_newer
_atomic_write = typos_rollout_cache.atomic_write
_DEFAULT_URLOPEN = urllib.request.urlopen

SCHEMA_VERSION = 1
HTTP_NOT_MODIFIED = 304
SUFFIX_PAIRS = (
    ("isably", "izably"),
    ("ise", "ize"),
    ("ises", "izes"),
    ("ised", "ized"),
    ("ising", "izing"),
    ("iser", "izer"),
    ("isers", "izers"),
    ("isable", "izable"),
    ("isation", "ization"),
    ("isations", "izations"),
)


@dc.dataclass(frozen=True)
class Dictionary:
    """Curated words and exclusions used to generate a ``typos`` config."""

    stems: tuple[str, ...] = ()
    accepted: tuple[str, ...] = ()
    corrections: tuple[tuple[str, str], ...] = ()
    phrase_corrections: tuple[tuple[str, str], ...] = ()
    ignore_patterns: tuple[str, ...] = ()
    excluded_files: tuple[str, ...] = ()


def _string_list(table: cabc.Mapping[str, object], key: str) -> tuple[str, ...]:
    """Read and validate a list of strings from a TOML table."""
    value = table.get(key, [])
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        message = f"{key!r} must be a list of strings"
        raise TypeError(message)
    return tuple(sorted(set(value)))


def _table(document: cabc.Mapping[str, object], key: str) -> cabc.Mapping[str, object]:
    """Read and validate a TOML table."""
    value = document.get(key, {})
    if not isinstance(value, dict):
        message = f"{key!r} must be a table"
        raise TypeError(message)
    return typ.cast("cabc.Mapping[str, object]", value)


def _string_mapping(
    table: cabc.Mapping[str, object],
    key: str,
    *,
    description: str,
) -> cabc.Mapping[str, str]:
    """Read and validate a string-to-string mapping from a TOML table."""
    value = _table(table, key)
    if not all(
        isinstance(item_key, str) and isinstance(item_value, str)
        for item_key, item_value in value.items()
    ):
        message = f"{description} must map strings to strings"
        raise TypeError(message)
    return typ.cast("cabc.Mapping[str, str]", value)


def _dictionary_from_text(text: str) -> Dictionary:
    """Parse and validate shared dictionary text."""
    document = tomllib.loads(text)
    schema = document.get("schema")
    if schema != SCHEMA_VERSION:
        message = f"unsupported dictionary schema {schema!r}"
        raise ValueError(message)
    oxford = _table(document, "oxford")
    words = _table(document, "words")
    phrases = _table(document, "phrases")
    patterns = _table(document, "patterns")
    files = _table(document, "files")
    corrections = _string_mapping(
        words,
        "corrections",
        description="word corrections",
    )
    phrase_corrections = _string_mapping(
        phrases,
        "corrections",
        description="phrase corrections",
    )
    return Dictionary(
        stems=_string_list(oxford, "stems"),
        accepted=_string_list(words, "accepted"),
        corrections=tuple(sorted(corrections.items())),
        phrase_corrections=tuple(sorted(phrase_corrections.items())),
        ignore_patterns=_string_list(patterns, "ignore"),
        excluded_files=_string_list(files, "exclude"),
    )


def load_dictionary(path: pathlib.Path) -> Dictionary:
    """Load a validated shared dictionary from a path.

    Parameters
    ----------
    path
        UTF-8 TOML dictionary to load.

    Returns
    -------
    Dictionary
        Validated spelling policy.

    Raises
    ------
    OSError
        If the dictionary cannot be read.
    TypeError, ValueError
        If the document does not satisfy the shared dictionary schema.
    """
    return _dictionary_from_text(path.read_text(encoding="utf-8"))


def _merge_correction_items(
    base: tuple[tuple[str, str], ...],
    local: tuple[tuple[str, str], ...],
    *,
    label: str,
) -> tuple[tuple[str, str], ...]:
    """Merge correction items while rejecting conflicting replacements."""
    merged = dict(base)
    for source, correction in local:
        existing = merged.get(source)
        if existing is not None and existing != correction:
            message = (
                f"conflicting {label} for {source!r}: {existing!r} != {correction!r}"
            )
            raise ValueError(message)
        merged[source] = correction
    return tuple(sorted(merged.items()))


def merge_dictionaries(base: Dictionary, local: Dictionary) -> Dictionary:
    """Merge a shared dictionary with a non-conflicting local overlay.

    Parameters
    ----------
    base
        Estate-wide spelling policy.
    local
        Repository-specific policy to add.

    Returns
    -------
    Dictionary
        Deterministically merged policy.

    Raises
    ------
    ValueError
        If the local policy conflicts with a shared correction.
    """
    return Dictionary(
        stems=tuple(sorted(set(base.stems) | set(local.stems))),
        accepted=tuple(sorted(set(base.accepted) | set(local.accepted))),
        corrections=_merge_correction_items(
            base.corrections,
            local.corrections,
            label="correction",
        ),
        phrase_corrections=_merge_correction_items(
            base.phrase_corrections,
            local.phrase_corrections,
            label="phrase correction",
        ),
        ignore_patterns=tuple(
            sorted(set(base.ignore_patterns) | set(local.ignore_patterns))
        ),
        excluded_files=tuple(
            sorted(set(base.excluded_files) | set(local.excluded_files))
        ),
    )


def generate_word_mappings(dictionary: Dictionary) -> dict[str, str]:
    """Expand Oxford stems and explicit words into deterministic mappings.

    Parameters
    ----------
    dictionary
        Validated spelling policy to expand.

    Returns
    -------
    dict[str, str]
        Sorted accepted-word identities and spelling corrections.

    Raises
    ------
    ValueError
        If generated and explicit corrections conflict.
    """
    mappings = {word: word for word in dictionary.accepted}

    def add(word: str, correction: str) -> None:
        existing = mappings.get(word)
        if existing is not None and existing != correction:
            message = (
                f"conflicting generated correction for {word!r}: "
                f"{existing!r} != {correction!r}"
            )
            raise ValueError(message)
        mappings[word] = correction

    for word, correction in dictionary.corrections:
        add(word, correction)
    for stem in dictionary.stems:
        for plain_british, oxford in SUFFIX_PAIRS:
            add(f"{stem}{plain_british}", f"{stem}{oxford}")
            add(f"{stem}{oxford}", f"{stem}{oxford}")
    return dict(sorted(mappings.items()))


def _toml_string(value: str) -> str:
    """Render a string using TOML-compatible JSON quoting."""
    return json.dumps(value, ensure_ascii=False)


def _render_array(name: str, values: tuple[str, ...]) -> list[str]:
    """Render a deterministic TOML string array."""
    lines = [f"{name} = ["]
    lines.extend(f"    {_toml_string(value)}," for value in values)
    lines.append("]")
    return lines


def render_typos_config(dictionary: Dictionary) -> str:
    """Render a deterministic, parse-checked ``typos.toml`` document.

    Parameters
    ----------
    dictionary
        Validated spelling policy to render.

    Returns
    -------
    str
        Parse-checked TOML configuration ending with a newline.

    Raises
    ------
    ValueError
        If generated and explicit corrections conflict.
    """
    lines = [
        "# Generated from the shared en-GB-oxendict dictionary.",
        "# Regenerate with scripts/generate_typos_config.py; do not edit by hand.",
        "",
        "[files]",
        *_render_array("extend-exclude", dictionary.excluded_files),
        "",
        "[default]",
        'locale = "en-gb"',
        *_render_array("extend-ignore-re", dictionary.ignore_patterns),
        "",
        "[default.extend-words]",
    ]
    lines.extend(
        f"{_toml_string(word)} = {_toml_string(correction)}"
        for word, correction in generate_word_mappings(dictionary).items()
    )
    rendered = "\n".join(lines) + "\n"
    tomllib.loads(rendered)
    return rendered


def write_config(path: pathlib.Path, dictionary: Dictionary) -> None:
    """Atomically write validated generated configuration to a path.

    Parameters
    ----------
    path
        Destination ``typos.toml`` path.
    dictionary
        Validated spelling policy to render and write.

    Raises
    ------
    OSError
        If the temporary file cannot be written or installed.
    ValueError
        If generated and explicit corrections conflict.
    """
    _atomic_write(path, render_typos_config(dictionary).encode())


def _validate_dictionary_bytes(content: bytes) -> None:
    """Reject bytes that do not contain a valid shared dictionary."""
    _dictionary_from_text(content.decode())


def _http_error_result(
    cache: pathlib.Path,
    error: urllib.error.HTTPError,
) -> RefreshResult:
    """Preserve the HTTP-status helper at the compatibility boundary."""
    return typos_rollout_http._http_error_result(
        cache,
        error,
        _validate_dictionary_bytes,
    )


def refresh_base(
    source: str | pathlib.Path,
    cache: pathlib.Path,
    options: RefreshOptions,
) -> RefreshResult:
    """Refresh an untracked base cache when the authoritative copy is newer.

    Parameters
    ----------
    source
        Local path or HTTPS URL for the authoritative shared dictionary.
    cache
        Untracked local cache destination.
    options
        Metadata, offline policy, and optional test transport.

    Returns
    -------
    RefreshResult
        Refresh status and validated cache path.

    Raises
    ------
    FileNotFoundError
        If offline mode has no valid cache.
    NetworkUnavailableError
        If the authority is unavailable and no valid stale cache exists.
    InsecureSourceError
        If an authority or redirect does not use HTTPS.
    OSError, urllib.error.HTTPError
        If persistence fails or the authority returns an HTTP error.
    TypeError, ValueError
        If dictionary validation fails.
    """
    selected_options = options
    if options.opener is None and urllib.request.urlopen is not _DEFAULT_URLOPEN:
        # Existing focused tests patch this legacy boundary; production retains
        # the HTTPS-only opener when the standard function is unchanged.
        selected_options = dc.replace(options, opener=urllib.request.urlopen)
    return typos_rollout_http.refresh_base(
        source,
        cache,
        typos_rollout_http._RefreshContext(
            selected_options,
            _validate_dictionary_bytes,
        ),
    )
