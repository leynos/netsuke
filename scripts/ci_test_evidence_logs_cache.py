"""Extract cache restore and compiler-cache observations from saved logs.

This adapter records exact, fallback, miss, and unknown states without
inferring cache warmth from a hit flag alone.
"""

import json
import re
import typing as typ

from ci_test_evidence_types import empty_evidence

if typ.TYPE_CHECKING:
    from ci_test_evidence_types import CacheEvidence

CACHE = re.compile(
    r"- (registry|tools|whitaker) key: `(netsuke-[A-Za-z0-9_.-]+)` hit: `(true|false)`$"
)
MAX_COUNTER = 10**18 - 1


def cache_records(lines: list[tuple[str | None, str]]) -> dict[str, CacheEvidence]:
    """Extract requested and restored cache keys from action log lines.

    Parameters
    ----------
    lines
        Timestamp and text pairs from one saved job log.

    Returns
    -------
    dict[str, CacheEvidence]
        Restore state, matched key, and known archive size for each cache.
    """
    caches = empty_evidence()["caches"]
    matched: dict[str, str] = {}
    missed: set[str] = set()
    archive_sizes: dict[str, int | None] = {}
    size = None
    for _, body in lines:
        size = _observed_archive_size(body, size)
        _record_restore(body, matched, archive_sizes, size)
        if body.startswith("Cache restored from key: "):
            size = None
        _record_miss(body, missed)
        _record_cache_key(body, caches, matched, missed)
    for cache in caches.values():
        cache["archive_bytes"] = archive_sizes.get(cache["matched_key"])
    return caches


def _observed_archive_size(body: str, current: int | None) -> int | None:
    """Return the current cache archive size, if a restore reports one."""
    if body.startswith("##[start-action"):
        return None
    match = re.fullmatch(
        r"Cache Size: ~\d{1,18}(?:\.\d{1,9})? [A-Za-z]+ \((\d{1,18}) B\)",
        body,
    )
    return int(match[1]) if match else current


def _record_restore(
    body: str,
    matched: dict[str, str],
    archive_sizes: dict[str, int | None],
    size: int | None,
) -> None:
    """Bind a restored cache key and size to its cache family."""
    restore = re.fullmatch(r"Cache restored from key: (netsuke-[A-Za-z0-9_.-]+)", body)
    if restore:
        key = restore[1]
        matched[key.split("-")[1]] = key
        archive_sizes[key] = size


def _record_miss(body: str, missed: set[str]) -> None:
    """Remember a requested key only when the restore action reported a miss."""
    miss = re.fullmatch(
        r"Cache not found for input keys: ((?:netsuke-[A-Za-z0-9_.-]+)"
        r"(?:, netsuke-[A-Za-z0-9_.-]+)*)",
        body,
    )
    if miss:
        missed.update(miss[1].split(", "))


def _record_cache_key(
    body: str,
    caches: dict[str, CacheEvidence],
    matched: dict[str, str],
    missed: set[str],
) -> None:
    """Resolve one requested cache key against restore output."""
    observation = CACHE.fullmatch(body)
    if observation:
        _cache_observation(observation, caches, matched, missed)


def _cache_observation(
    observation: re.Match[str],
    caches: dict[str, CacheEvidence],
    matched: dict[str, str],
    missed: set[str],
) -> None:
    """Resolve exact, fallback, miss, and incomplete cache output states."""
    name, key, hit = observation.groups()
    match_key = matched.get("cargo" if name == "registry" else name)
    state = _cache_state(key, hit, match_key, missed)
    caches[name].update(requested_key=key, matched_key=match_key, state=state)


def _cache_state(key: str, hit: str, match_key: str | None, missed: set[str]) -> str:
    """Classify one cache key without treating missing evidence as a hit."""
    if match_key is not None:
        return "exact" if key == match_key else "fallback"
    if key in missed:
        return "miss"
    return "exact" if hit == "true" else "non_exact_unknown"


def sccache_records(lines: list[tuple[str | None, str]]) -> dict[str, int | None]:
    """Extract compiler-cache counters from text and JSON log lines.

    Parameters
    ----------
    lines
        Timestamp and text pairs from one saved job log.

    Returns
    -------
    dict[str, int | None]
        Observed integer counters; missing statistics remain ``None``.
    """
    result = empty_evidence()["sccache"]
    for _, body in lines:
        _record_sccache_line(body, result)
    return result


def _record_sccache_line(body: str, result: dict[str, int | None]) -> None:
    """Read one text or JSON sccache counter line."""
    counter = re.fullmatch(r"Cache (hits|misses)\s+(\d{1,18})", body)
    if counter:
        result[counter[1]] = int(counter[2])
    elif body.startswith('{"stats":'):
        _record_sccache_json(body, result)


def _record_sccache_json(body: str, result: dict[str, int | None]) -> None:
    """Copy only non-negative integer counters from sccache JSON."""
    try:
        stats = json.loads(body).get("stats", {})
    except ValueError, RecursionError:
        return
    match stats:
        case dict() as counters:
            result.update({
                key: scalar
                for key, scalar in counters.items()
                if key in result and type(scalar) is int and 0 <= scalar <= MAX_COUNTER
            })
        case _:
            return
