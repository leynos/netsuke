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

#: One cache action, one pin, on every lane. Ubicloud's transparent cache
#: intercepts `actions/cache` at v6.1.0, verified from the Ubicloud console
#: listing on 2026-09-03, so the deprecated `ubicloud/cache` fork is not used
#: and v4 pins are not either: v4.3.0 left nothing in the Ubicloud store.
CACHE_PIN = "55cc8345863c7cc4c66a329aec7e433d2d1c52a9"
CACHE_RESTORE = f"actions/cache/restore@{CACHE_PIN}"
CACHE_SAVE = f"actions/cache/save@{CACHE_PIN}"
EXTERNAL_CACHE_PROVIDER = "external"
SETUP_RUST_ACTION = "leynos/shared-actions/.github/actions/setup-rust@"
SCCACHE_CREDENTIALS_ACTION = "./.github/actions/sccache-gha-credentials"

#: Cargo's build tree is archived by no cache step anywhere in this
#: repository. sccache is the single owner of compiler output for every build
#: shape, so a `target` archive would be a second owner of the same bytes and
#: would be invalidated far more often than it helped. Kani's directories are
#: a tool payload rather than a build tree, so they are not matched here.
FORBIDDEN_CACHE_PATHS = ("target", "target/")

#: Every source of cache steps. Ubicloud lanes and GitHub-hosted lanes are
#: listed separately because their keys differ, not their provider.
UBICLOUD_CACHE_SOURCES = (
    (ACTION_DIR / "linux-gate-cache" / "action.yml", None),
    (ACTION_DIR / "kani-cache" / "action.yml", None),
    (WORKFLOW_DIR / "netsukefile-test.yml", "netsukefile"),
    (WORKFLOW_DIR / "coverage-main.yml", "coverage-upload"),
)
GITHUB_CACHE_SOURCES = ((ACTION_DIR / "windows-gate-cache" / "action.yml", None),)

#: Ubicloud lanes whose sccache server must be credentialed before it starts.
#: A `run` step on Ubicloud cannot see `ACTIONS_RESULTS_URL` or
#: `ACTIONS_RUNTIME_TOKEN`, so a server started first stays on local disk for
#: the whole job and reports zero compile requests.
SCCACHE_CREDENTIAL_JOBS = (
    ("ci.yml", "build-test"),
    ("netsukefile-test.yml", "netsukefile"),
    ("coverage-main.yml", "coverage-upload"),
    ("ci-windows.yml", "build-test-windows"),
    ("ci-windows.yml", "windows-native-recipe-smoke"),
    ("release.yml", "windows-native-recipe-smoke"),
)

#: Jobs that compile Rust and must therefore reach the compiler cache. The
#: packaging job takes its sccache from the nested shared build action, so it
#: sets the wrapper without installing a second one.
SCCACHE_WRAPPER_JOBS = (
    ("ci.yml", "build-test"),
    ("ci-windows.yml", "build-test-windows"),
    ("ci-windows.yml", "windows-native-recipe-smoke"),
    ("netsukefile-test.yml", "netsukefile"),
    ("coverage-main.yml", "coverage-upload"),
    ("release.yml", "windows-native-recipe-smoke"),
    ("build-and-package.yml", "build"),
)

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
