"""Extract allow-listed native facts from saved CI logs, without copying errors.

This offline adapter belongs only to the CI investigation collector. Unknown
formats stay missing; command echoes are not evidence of completed execution.
"""

import json
import re
import typing as typ

from ci_test_evidence_nextest import nextest_phase_marker, parse_nextest_records
from ci_test_evidence_types import empty_evidence

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from ci_test_evidence_types import (
        CacheEvidence,
        DoctestEvidence,
        DoctestSection,
        LogEvidence,
    )

MAX_PERCENT = 100
ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
TIMESTAMP = re.compile(r"(?P<time>\d{4}-\d{2}-\d{2}T[\d:.]+Z) (?P<body>.*)$")
CACHE = re.compile(
    r"- (registry|tools|whitaker) key: `(netsuke-[A-Za-z0-9_.-]+)` hit: `(true|false)`$"
)
RESULT = re.compile(
    r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;"
)
ENVIRONMENT = frozenset({
    "RUSTFLAGS",
    "RUSTDOCFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "LLVM_PROFILE_FILE",
    "LLVM_PROFILE_FILE_NAME",
    "CARGO_BUILD_JOBS",
    "NEXTEST_TEST_THREADS",
    "NEXTEST_PROFILE",
    "NETSUKE_RUST_TOOLCHAIN",
    "BUILD_PROFILE",
    "RUN_RUST_CARGO_WAIT_TIMEOUT",
    "LINUX_LANE_VCPUS",
})
ACTION = re.compile(
    r"(?:Download action repository ')?"
    r"([A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+)@([a-f0-9]{40})"
)


def _lines(log: str) -> cabc.Iterator[tuple[str | None, str]]:
    """Split native lines; ``_lines("noise")`` yields ``(None, "noise")``."""
    for line in log.splitlines():
        clean = ANSI.sub("", line).lstrip("\ufeff")
        match = TIMESTAMP.search(clean)
        yield (match["time"], match["body"]) if match else (None, clean)


def _cache_records(lines: list[tuple[str | None, str]]) -> dict[str, CacheEvidence]:
    """Read restores; an old restored key for a new key means fallback."""
    caches = empty_evidence()["caches"]
    matched = {}
    missed = set()
    archive_sizes = {}
    size = None
    for _, body in lines:
        if body.startswith("##[start-action"):
            size = None
        size_line = re.fullmatch(r"Cache Size: ~[\d.]+ [A-Za-z]+ \((\d+) B\)", body)
        if size_line:
            size = int(size_line[1])
        restore = re.fullmatch(
            r"Cache restored from key: (netsuke-[A-Za-z0-9_.-]+)", body
        )
        miss = re.fullmatch(
            r"Cache not found for input keys: (netsuke-[A-Za-z0-9_.-]+)"
            r"(?:, netsuke-[A-Za-z0-9_.-]+)*",
            body,
        )
        if restore:
            matched[restore[1].split("-")[1]] = restore[1]
            archive_sizes[restore[1]] = size
            size = None
        if miss:
            missed.add(miss[1])
        observation = CACHE.fullmatch(body)
        if observation:
            _cache_observation(observation, caches, matched, missed)
    for cache in caches.values():
        cache["archive_bytes"] = archive_sizes.get(cache["matched_key"])
    return caches


def _cache_observation(
    observation: re.Match,
    caches: dict[str, CacheEvidence],
    matched: dict[str, str],
    missed: set[str],
) -> None:
    """Resolve hits; a restored ``old`` key for ``new`` gives fallback."""
    name, key, hit = observation.groups()
    match_key = matched.get("cargo" if name == "registry" else name)
    state = "exact" if hit == "true" else "non_exact_unknown"
    if match_key is not None:
        state = "exact" if key == match_key else "fallback"
    elif key in missed:
        state = "miss"
    caches[name].update(requested_key=key, matched_key=match_key, state=state)


def _closes_doctest_scope(body: str, target: str | None, completed: str | None) -> bool:
    """Close a command or completed target, retaining a newly announced target.

    Returns
    -------
    bool
        For example, ``_closes_doctest_scope("$ cargo test", None, None)`` is true.
    """
    boundary = body.startswith(("##[endgroup]", "$ ", "[command]"))
    finished_target = body.startswith("all doctests ran in ") and target == completed
    return boundary or finished_target


def _doctest_records(lines: list[tuple[str | None, str]]) -> DoctestEvidence:
    """Count Doc-tests; unmarked ``test result: ok`` remains unknown."""
    sections: list[DoctestSection] = []
    current: DoctestSection | None = None
    target: str | None = None
    completed_target: str | None = None
    for timestamp, body in lines:
        if _closes_doctest_scope(body, target, completed_target):
            target, current, completed_target = None, None, None
        marker = re.fullmatch(r"\s*Doc-tests ([A-Za-z0-9_-]+)", body)
        if marker:
            # Cargo can announce the next target before the active batch ends.
            target = marker[1]
        batch = re.fullmatch(r"running \d+ tests?", body)
        starts_batch = marker is not None or batch is not None
        # Rustdoc emits compile-fail cases as another batch for the same target.
        needs_batch = current is None and starts_batch
        if target is not None and needs_batch:
            current = {
                "target": target,
                "started_at": timestamp,
                "ended_at": None,
                "counts": {},
            }
        result = RESULT.match(body)
        if result and current is not None:
            current.update(
                ended_at=timestamp,
                counts=dict(
                    zip(
                        ("passed", "failed", "ignored"),
                        map(int, result.groups()),
                        strict=True,
                    )
                ),
            )
            sections.append(current)
            completed_target = current["target"]
            current = None
    counts = (
        {
            key: sum(section["counts"][key] for section in sections)
            for key in ("passed", "failed", "ignored")
        }
        if sections
        else None
    )
    return {"counts": counts, "sections": sections}


def _sccache_records(lines: list[tuple[str | None, str]]) -> dict[str, int | None]:
    """Copy counters; ``Cache hits 2`` yields ``hits=2``."""
    result = empty_evidence()["sccache"]
    for _, body in lines:
        value = re.fullmatch(r"Cache (hits|misses)\s+(\d+)", body)
        if value:
            result[value[1]] = int(value[2])
        if body.startswith('{"stats":'):
            try:
                stats = json.loads(body).get("stats", {})
            except json.JSONDecodeError:
                continue
            if not isinstance(stats, dict):
                continue
            for key in result:
                scalar = stats.get(key)
                if type(scalar) is int and scalar >= 0:
                    result[key] = scalar
    return result


def _native_records(lines: list[tuple[str | None, str]], evidence: LogEvidence) -> None:
    """Read provenance; ``rustc 1.2.3`` records the observed version."""
    groups: list[str] = []
    step_environment: dict[str, str] = {}
    for index, (timestamp, body) in enumerate(lines):
        if body.startswith("##[group]Run "):
            step_environment = {}
        in_runner_image = _inside_runner_image_group(body, groups)
        if "[command]/usr/bin/git log -1 --format=%H" in body and index + 1 < len(
            lines
        ):
            sha = lines[index + 1][1]
            if re.fullmatch(r"[a-f0-9]{40}", sha):
                evidence["tested_merge_sha"] = sha
        for action, sha in ACTION.findall(body):
            evidence["action_pins"][action] = sha
        environment = re.fullmatch(r"\s+([A-Z_]+): ([A-Za-z0-9_./% =:+,;\x1f-]+)", body)
        if environment and environment[1] in ENVIRONMENT:
            evidence["environment"][environment[1]] = environment[2]
            step_environment[environment[1]] = environment[2]
        _version_record(body, evidence)
        _selection_record(body, evidence, step_environment)
        _build_duration_record(timestamp, body, evidence)
        _coverage_record(body, evidence)
        _resource_record(body, evidence, in_runner_image=in_runner_image)
        phase = _phase_marker(body)
        if phase:
            evidence["native_phases"].append({"phase": phase, "timestamp": timestamp})


def _inside_runner_image_group(body: str, groups: list[str]) -> bool:
    """Track log groups and identify fields scoped to the Runner Image group."""
    if body.startswith("##[group]"):
        groups.append(body.removeprefix("##[group]"))
    in_runner_image = "Runner Image" in groups
    if body == "##[endgroup]" and groups:
        groups.pop()
    return in_runner_image


def _version_record(body: str, evidence: LogEvidence) -> None:
    """Read versions; native ``rustc 1.2.3`` records version ``1.2.3``."""
    version = re.fullmatch(
        r"(rustc|cargo|cargo-nextest) ([0-9][A-Za-z0-9 .()+-]+)", body
    )
    if version:
        key = "nextest" if version[1] == "cargo-nextest" else version[1]
        evidence["versions"][key] = version[2]
    llvm_cov = re.fullmatch(r"cargo-llvm-cov ([\d.]+) installed to .+", body)
    if llvm_cov:
        evidence["versions"]["cargo_llvm_cov"] = llvm_cov[1]
    llvm = re.fullmatch(r"LLVM version: ([0-9.]+)", body)
    if llvm:
        evidence["versions"]["llvm"] = llvm[1]
    if body.startswith('{"stats":'):
        sccache = re.search(r'"version":"([0-9.]+)"', body)
        if sccache:
            evidence["versions"]["sccache"] = sccache[1]


def _selection_record(
    body: str, evidence: LogEvidence, step_environment: dict[str, str]
) -> None:
    """Read flags; a coverage command with ``--all-features`` sets true."""
    if body.startswith("$ cargo llvm-cov nextest "):
        evidence["coverage_environment"] = step_environment.copy()
        evidence["coverage_selection"].update({
            key: flag in body.split()
            for key, flag in (
                ("workspace", "--workspace"),
                ("all_targets", "--all-targets"),
                ("all_features", "--all-features"),
            )
        })
    if body.startswith("$ cargo test --doc "):
        evidence["coverage_selection"]["doctests"] = True


def _build_duration_record(
    timestamp: str | None, body: str, evidence: LogEvidence
) -> None:
    """Read Cargo timing; ``in 1m 2s`` on a Finished line gives 62s."""
    finished = re.fullmatch(
        r"\s*Finished `(test|dev|release)` profile .* in "
        r"(?:(\d{1,9})m )?(\d{1,9}(?:\.\d{1,9})?)s",
        body,
    )
    if finished:
        evidence["build_profile_durations"].append({
            "profile": finished[1],
            "seconds": int(finished[2] or 0) * 60 + float(finished[3]),
            "timestamp": timestamp,
        })


def _coverage_record(body: str, evidence: LogEvidence) -> None:
    """Read ratchets; ``Current coverage: 90%`` records current percent."""
    coverage = re.fullmatch(
        r"(Current|Baseline) coverage: (\d{1,3}(?:\.\d{1,9})?)%", body
    )
    if coverage and float(coverage[2]) <= MAX_PERCENT:
        evidence["coverage"][f"{coverage[1].lower()}_percent"] = float(coverage[2])


def _resource_record(
    body: str, evidence: LogEvidence, *, in_runner_image: bool
) -> None:
    """Read used memory; ``- memory: 3 MiB on 4 vCPUs`` records 3 MiB."""
    memory = re.fullmatch(r"- memory: (\d+) MiB on (\d+) vCPUs", body)
    disk = re.fullmatch(r"- disk used: (\d+) MiB, least free: (\d+) MiB", body)
    if memory:
        evidence["resources"].update(
            system_used_peak_mib=int(memory[1]), vcpus=int(memory[2])
        )
    if disk:
        evidence["resources"].update(
            disk_used_peak_mib=int(disk[1]), disk_least_free_mib=int(disk[2])
        )
    if in_runner_image:
        for prefix, field in (
            ("Image: ", "image"),
            ("Version: ", "image_version"),
            ("Arch: ", "architecture"),
        ):
            if body.startswith(prefix) and re.fullmatch(
                r"[A-Za-z0-9_.-]+", body[len(prefix) :]
            ):
                evidence["runner"][field] = body[len(prefix) :]


def _phase_marker(body: str) -> str | None:
    """Map markers; ``_phase_marker("noise")`` returns ``None``."""
    if body.startswith("$ cargo llvm-cov nextest "):
        return "instrumented_build_started"
    if body.startswith("$ cargo test --doc "):
        return "doctest_command_started"
    if phase := nextest_phase_marker(body):
        return phase
    if re.fullmatch(r"\s*Doc-tests [A-Za-z0-9_-]+", body):
        return "doctest_target_started"
    return None


def parse_log(log: str) -> LogEvidence:
    """Return safe extracted evidence; unobserved facts remain null.

    Returns
    -------
    LogEvidence
        For example, empty logs leave ``nextest.seconds=None``.
    """
    lines = list(_lines(log))
    evidence = empty_evidence()
    evidence.update(
        caches=_cache_records(lines),
        nextest=parse_nextest_records(lines),
        doctests=_doctest_records(lines),
        sccache=_sccache_records(lines),
    )
    _native_records(lines, evidence)
    return evidence
