"""Shared data and step helpers for the cache-contract suites.

`cache_ownership_test.py`, `cache_write_policy_test.py`, and
`sccache_contract_test.py` read the same workflow and composite-action files
and need the same view of them: which lane owns which paths, which provider
each lane must use, and which job writes each key family. Those tables live
here so each suite states its contract rather than restating the inventory,
and so a placement change has one place to update. This module holds no tests
of its own.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import yaml
from workflow_loading import (
    REPO_ROOT,
    job_steps,
    load_workflow,
    require_list,
    require_mapping,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

WORKFLOW_DIR = REPO_ROOT / ".github" / "workflows"
ACTION_DIR = REPO_ROOT / ".github" / "actions"

UBICLOUD_CACHE_PIN = "92361f338d82d2c58a98875f1b5c95cd14cd6b2a"
GITHUB_CACHE_PIN = "55cc8345863c7cc4c66a329aec7e433d2d1c52a9"
UBICLOUD_RESTORE = f"ubicloud/cache/restore@{UBICLOUD_CACHE_PIN}"
UBICLOUD_SAVE = f"ubicloud/cache/save@{UBICLOUD_CACHE_PIN}"
GITHUB_RESTORE = f"actions/cache/restore@{GITHUB_CACHE_PIN}"
GITHUB_SAVE = f"actions/cache/save@{GITHUB_CACHE_PIN}"
EXTERNAL_CACHE_PROVIDER = "external"
SETUP_RUST_ACTION = "leynos/shared-actions/.github/actions/setup-rust@"

#: Every source of cache steps, paired with the provider its runner requires.
#: `ubicloud/cache` reads runtime variables that only a Ubicloud VM supplies,
#: so it must never appear in a job that can run on a GitHub-hosted label, and
#: the reverse archive would never reach the Ubicloud store.
UBICLOUD_CACHE_SOURCES = (
    (ACTION_DIR / "linux-gate-cache" / "action.yml", None),
    (ACTION_DIR / "kani-cache" / "action.yml", None),
    (WORKFLOW_DIR / "netsukefile-test.yml", "netsukefile"),
    (WORKFLOW_DIR / "coverage-main.yml", "coverage-upload"),
)
GITHUB_CACHE_SOURCES = ((ACTION_DIR / "windows-gate-cache" / "action.yml", None),)

#: Jobs that call a cache action rather than declaring cache steps inline.
#: They must reference the composite for their platform and no other.
CACHE_ACTION_CALLERS = {
    ("ci.yml", "build-test"): "./.github/actions/linux-gate-cache",
    ("ci.yml", "kani-smoke"): "./.github/actions/kani-cache",
    ("ci-windows.yml", "build-test-windows"): "./.github/actions/windows-gate-cache",
    ("ci-windows.yml", "windows-native-recipe-smoke"): (
        "./.github/actions/windows-gate-cache"
    ),
    ("release.yml", "windows-native-recipe-smoke"): (
        "./.github/actions/windows-gate-cache"
    ),
}

#: Jobs that must publish their rendered keys and hit results on every run,
#: paired with the file that declares the observation step.
OBSERVATION_SOURCES = (
    (ACTION_DIR / "linux-gate-cache" / "action.yml", None),
    (ACTION_DIR / "kani-cache" / "action.yml", None),
    (ACTION_DIR / "windows-gate-cache" / "action.yml", None),
    (WORKFLOW_DIR / "netsukefile-test.yml", "netsukefile"),
    (WORKFLOW_DIR / "coverage-main.yml", "coverage-upload"),
)

#: Workflows whose caches need a trunk run to publish a generation at all.
TRUNK_TRIGGERED_WORKFLOWS = ("ci.yml", "netsukefile-test.yml", "coverage-main.yml")

#: The single writer designated for each key family, and the jobs that may
#: only read it. A second writer would race for the reservation and make warm
#: behaviour depend on which job finished first.
KEY_WRITERS = {
    "cargo (ubuntu 24.04)": ("ci.yml", "build-test"),
    "cargo (ubuntu 22.04)": ("netsukefile-test.yml", "netsukefile"),
    "cargo (windows)": ("ci-windows.yml", "build-test-windows"),
    "kani": ("ci.yml", "kani-smoke"),
}
READ_ONLY_CACHE_JOBS = (("coverage-main.yml", "coverage-upload"),)

#: Jobs that call a cache composite with the read-only profile.
SMOKE_PROFILE_JOBS = (
    ("ci-windows.yml", "windows-native-recipe-smoke"),
    ("release.yml", "windows-native-recipe-smoke"),
)


def lane_steps(source: Path, job_name: str | None) -> list[dict[str, object]]:
    """Return the steps of a workflow job or of a composite action."""
    if job_name is not None:
        return job_steps(load_workflow(source), job_name)
    document = yaml.safe_load(source.read_text(encoding="utf-8"))
    runs = require_mapping(require_mapping(document, source.name).get("runs"), "runs")
    return [
        require_mapping(step, f"{source.name} step")
        for step in require_list(runs.get("steps"), f"{source.name} steps")
    ]


def cache_steps(steps: list[dict[str, object]]) -> list[dict[str, object]]:
    """Return every step that reserves, reads, or writes a cache archive."""
    return [
        step
        for step in steps
        if "/cache/" in str(step.get("uses", ""))
        or str(step.get("uses", "")).endswith("/cache")
    ]


def declared_paths(step: dict[str, object]) -> list[str]:
    """Return the cached paths a cache step claims, one per line."""
    inputs = require_mapping(step.get("with"), "cache step inputs")
    return [
        line.strip()
        for line in str(inputs.get("path", "")).splitlines()
        if line.strip()
    ]
