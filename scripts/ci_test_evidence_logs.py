"""Extract allow-listed native facts from saved CI logs, without copying errors.

This offline adapter belongs only to the CI investigation collector. Unknown
formats stay missing; command echoes are not evidence of completed execution.
"""

import dataclasses as dc
import json
import re
import typing as typ

from ci_test_evidence_logs_cache import cache_records, sccache_records
from ci_test_evidence_logs_doctests import doctest_records
from ci_test_evidence_nextest import nextest_phase_marker, parse_nextest_records
from ci_test_evidence_types import empty_evidence

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from ci_test_evidence_types import (
        LogEvidence,
    )

MAX_PERCENT = 100
ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")
TIMESTAMP = re.compile(r"(?P<time>\d{4}-\d{2}-\d{2}T[\d:.]+Z) (?P<body>.*)$")
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


def _native_records(lines: list[tuple[str | None, str]], evidence: LogEvidence) -> None:
    """Read provenance; ``rustc 1.2.3`` records the observed version."""
    context = _NativeLogContext(lines=lines, evidence=evidence)
    for index in range(len(lines)):
        context.consume(index)


@dc.dataclass(slots=True)
class _NativeLogContext:
    """Carry log parsing state without widening line-handler arguments."""

    lines: list[tuple[str | None, str]]
    evidence: LogEvidence
    groups: list[str] = dc.field(default_factory=list)
    step_environment: dict[str, str] = dc.field(default_factory=dict)
    in_runner_image: bool = False

    def consume(self, index: int) -> None:
        """Record the evidence fields carried by one native log line."""
        timestamp, body = self.lines[index]
        if body.startswith("##[group]Run "):
            self.step_environment.clear()
        self.in_runner_image = _inside_runner_image_group(body, self.groups)
        _tested_sha_record(self.lines, index, body, self.evidence)
        _action_pin_records(body, self.evidence)
        _environment_record(body, self.step_environment, self.evidence)
        _native_measurements(timestamp, body, self)


def _tested_sha_record(
    lines: list[tuple[str | None, str]],
    index: int,
    body: str,
    evidence: LogEvidence,
) -> None:
    """Capture the checkout SHA printed immediately after the git command."""
    if "[command]/usr/bin/git log -1 --format=%H" not in body:
        return
    if index + 1 < len(lines) and re.fullmatch(r"[a-f0-9]{40}", lines[index + 1][1]):
        evidence["tested_merge_sha"] = lines[index + 1][1]


def _action_pin_records(body: str, evidence: LogEvidence) -> None:
    """Record immutable action references present in a runner log line."""
    for action, sha in ACTION.findall(body):
        evidence["action_pins"][action] = sha


def _environment_record(
    body: str, step_environment: dict[str, str], evidence: LogEvidence
) -> None:
    """Record only allow-listed environment values without exposing secrets."""
    environment = re.fullmatch(r"\s+([A-Z_]+): ([A-Za-z0-9_./% =:+,;\x1f-]+)", body)
    if environment and environment[1] in ENVIRONMENT:
        evidence["environment"][environment[1]] = environment[2]
        step_environment[environment[1]] = environment[2]


def _native_measurements(
    timestamp: str | None,
    body: str,
    context: _NativeLogContext,
) -> None:
    """Apply independent version, coverage, resource, and phase parsers."""
    _version_record(body, context.evidence)
    _selection_record(body, context.evidence, context.step_environment)
    _build_duration_record(timestamp, body, context.evidence)
    _coverage_record(body, context.evidence)
    _resource_record(body, context.evidence, in_runner_image=context.in_runner_image)
    if phase := _phase_marker(body):
        context.evidence["native_phases"].append({
            "phase": phase,
            "timestamp": timestamp,
        })


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
    sccache_version = _sccache_version(body)
    if sccache_version:
        evidence["versions"]["sccache"] = sccache_version


def _sccache_version(body: str) -> str | None:
    """Extract a version only from the structured sccache stats line."""
    if not body.startswith('{"stats":'):
        return None
    try:
        payload = json.loads(body)
    except json.JSONDecodeError:
        return None
    if not isinstance(payload, dict):
        return None
    version = payload.get("version")
    if version is None and isinstance(payload.get("stats"), dict):
        version = payload["stats"].get("version")
    return (
        version
        if isinstance(version, str) and re.fullmatch(r"[0-9.]+", version)
        else None
    )


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

    Parameters
    ----------
    log
        Raw text of one saved job log, with or without timestamp prefixes.

    Returns
    -------
    LogEvidence
        For example, empty logs leave ``nextest.seconds=None``.
    """
    lines = list(_lines(log))
    evidence = empty_evidence()
    evidence.update(
        caches=cache_records(lines),
        nextest=parse_nextest_records(lines),
        doctests=doctest_records(lines),
        sccache=sccache_records(lines),
    )
    _native_records(lines, evidence)
    return evidence
