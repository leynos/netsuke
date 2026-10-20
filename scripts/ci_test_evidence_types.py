"""Define fixed collector vocabularies, serialization shapes, and validation.

Only the offline collector and its log adapter own these records. They are not
application interfaces, and they must never carry arbitrary log or error text.
"""

import re
import typing as typ

RUNNER_LABELS = frozenset({
    "ubuntu-latest",
    "windows-latest",
    "macos-latest",
    "self-hosted",
    "Linux",
    "X64",
    "ARM64",
    "Windows",
    "macOS",
})
UBICLOUD_SHAPE = re.compile(r"ubicloud-[a-z0-9]+-\d{1,2}-(?:[a-z0-9]+-)+\d{4}")
OUTCOMES = frozenset({
    "success",
    "failure",
    "cancelled",
    "skipped",
    "timed_out",
    "neutral",
    "action_required",
    "stale",
})
PHASES = {
    "Restore gate caches": "cache_restore",
    "Generate coverage": "coverage_combined",
    "Test and Measure Coverage": "coverage_combined",
    "Test with coverage": "coverage_combined",
    "Show sccache statistics": "compiler_cache_observation",
    "Report peak resource use": "resource_observation",
    "Save gate caches": "cache_save",
    "Validate coverage artefact": "coverage_validation",
    "Upload coverage data to CodeScene": "codescene_upload",
}


def validate_identities(identities: list[object]) -> None:
    """Require positive integer identities at the offline API boundary.

    For example, ``validate_identities([True])`` raises ``TypeError``.

    Raises
    ------
    TypeError
        If an identity is not an integer or is a boolean.
    ValueError
        If an integer identity is not positive.
    """
    for value in identities:
        if not isinstance(value, int) or isinstance(value, bool):
            message = "invalid integer identity"
            raise TypeError(message)
        if value <= 0:
            message = "invalid positive identity"
            raise ValueError(message)


def normalize_step_number(value: object) -> int | None:
    """Normalize a step number; ``normalize_step_number(True)`` returns ``None``."""
    match value:
        case bool():
            return None
        case int() if value >= 0:
            return value
        case _:
            return None


class CacheEvidence(typ.TypedDict):
    """Describe observed archive identity without inferring warmth."""

    requested_key: str | None
    matched_key: str | None
    state: str
    archive_bytes: int | None


class TestEvidence(typ.TypedDict):
    """Retain a separately identifiable native test outcome."""

    identity: str
    outcome: str
    seconds: float | None
    ended_at: str | None


class NextestEvidence(typ.TypedDict):
    """Keep wall time separate from overlapping individual test durations."""

    seconds: float | None
    selected_count: int | None
    discovered_count: int | None
    executed_count: int | None
    skipped_count: int | None
    outcomes: dict[str, int] | None
    tests: list[TestEvidence]
    identity_inventory_complete: bool
    executed_identity_inventory_complete: bool
    retries: int | None


class DoctestSection(typ.TypedDict):
    """Bind counters to one observed native documentation-test target."""

    target: str
    started_at: str | None
    ended_at: str | None
    counts: dict[str, int]


class DoctestEvidence(typ.TypedDict):
    """Record doctest counts without including nested test harness output."""

    counts: dict[str, int] | None
    sections: list[DoctestSection]


class LogEvidence(typ.TypedDict):
    """Hold fixed evidence fields; null denotes unavailable measurements."""

    versions: dict[str, str | None]
    runner: dict[str, str | int | None]
    environment: dict[str, str]
    coverage_environment: dict[str, str]
    coverage_selection: dict[str, bool | None]
    action_pins: dict[str, str]
    tested_merge_sha: str | None
    caches: dict[str, CacheEvidence]
    sccache: dict[str, int | None]
    nextest: NextestEvidence
    doctests: DoctestEvidence
    resources: dict[str, int | float | None]
    native_phases: list[dict[str, str | None]]
    build_profile_durations: list[dict[str, str | float | None]]
    nested_cargo: dict[str, int | float | None]
    profiles: dict[str, int | float | None]
    coverage: dict[str, float | str | None]


class AttemptCost(typ.TypedDict, total=False):
    """Describe aggregate attempt consumption and explicitly unknown billing."""

    attempt_runner_seconds: float | None
    attempt_observed_runner_seconds: float
    attempt_unknown_duration_jobs: int
    attempt_ubicloud_runner_seconds: float | None
    attempt_ubicloud_seconds_by_shape: dict[str, float | None]
    cost_basis: str
    billing_consumption: None
    storage_transfer_bytes: int | None


class JobRecord(AttemptCost):
    """Serialize one job without mixing source and execution provenance."""

    schema: int
    run_id: int
    job_id: int
    run_attempt: int
    run_url: str
    job_url: str
    event: str
    source_sha: str | None
    tested_merge_sha: str | None
    variant: str
    relevant_diff: str | None
    comparison: str
    cohort: str
    runner_labels: list[str]
    attempt_outcome: str
    outcome: str
    started_at: str | None
    completed_at: str | None
    whole_job_seconds: float | None
    attempt_start_to_job_start_seconds: float | None
    queue_and_provisioning_seconds: float | None
    steps: list[dict[str, object]]
    evidence: LogEvidence
    log_available: bool
    telemetry_available: bool


def empty_evidence() -> LogEvidence:
    """Create unknown fields; for example, ``tested_merge_sha`` is ``None``."""
    return {
        "versions": dict.fromkeys((
            "rustc",
            "cargo",
            "llvm",
            "nextest",
            "cargo_llvm_cov",
            "sccache",
        )),
        "runner": dict.fromkeys((
            "image",
            "image_version",
            "architecture",
            "cpu_model",
            "memory_capacity_mib",
        )),
        "environment": {},
        "coverage_environment": {},
        "coverage_selection": dict.fromkeys((
            "workspace",
            "all_targets",
            "all_features",
            "doctests",
        )),
        "action_pins": {},
        "tested_merge_sha": None,
        "caches": {
            key: {
                "requested_key": None,
                "matched_key": None,
                "state": "unknown",
                "archive_bytes": None,
            }
            for key in ("registry", "tools", "whitaker")
        },
        "sccache": dict.fromkeys((
            "hits",
            "misses",
            "compile_requests",
            "compilations",
            "compile_fails",
            "requests_not_cacheable",
        )),
        "nextest": {
            "seconds": None,
            "selected_count": None,
            "discovered_count": None,
            "executed_count": None,
            "skipped_count": None,
            "outcomes": None,
            "tests": [],
            "identity_inventory_complete": False,
            "executed_identity_inventory_complete": False,
            "retries": None,
        },
        "doctests": {"counts": None, "sections": []},
        "resources": dict.fromkeys((
            "system_used_peak_mib",
            "vcpus",
            "disk_used_peak_mib",
            "disk_least_free_mib",
            "peak_rss_mib",
            "cpu_seconds",
            "io_bytes",
        )),
        "native_phases": [],
        "build_profile_durations": [],
        "nested_cargo": {
            "invocations": None,
            "fresh_artifacts": None,
            "rebuilt_artifacts": None,
            "lock_wait_seconds": None,
            "exclusive_tail_seconds": None,
        },
        "profiles": dict.fromkeys((
            "raw_count",
            "raw_bytes",
            "profdata_bytes",
            "object_count",
            "merge_seconds",
            "export_seconds",
        )),
        "coverage": dict.fromkeys((
            "current_percent",
            "baseline_percent",
            "covered_lines",
            "total_lines",
            "detailed_report_sha256",
        )),
    }
