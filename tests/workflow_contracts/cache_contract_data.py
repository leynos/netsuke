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

import re
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
#: Local actions that own no cache, so a job may call them alongside its lane
#: cache action without owning two.
NON_CACHE_ACTIONS = (
    "./.github/actions/install-mdtablefix",
    "./.github/actions/memory-sampler",
)

#: Cargo's build tree is archived by no cache step anywhere in this
#: repository. sccache is the single owner of compiler output for every build
#: shape, so a `target` archive would be a second owner of the same bytes and
#: would be invalidated far more often than it helped. Kani's directories are
#: a tool payload rather than a build tree, so they are not matched here.
#: Matched as a path prefix rather than exactly: `target`, `target/`, and
#: `target/debug` are all build-tree archives, and an exact-match list would
#: have admitted every one but the first two.
FORBIDDEN_CACHE_PATHS = ("target",)

#: Any reference to the cache action, combined or split. Matching only
#: `/cache/` missed the combined `actions/cache@...` form, which would have
#: made every ownership contract pass over a real second owner in silence.
CACHE_ACTION_PATTERN = re.compile(r"/cache(?:/(?:restore|save))?@")


def is_build_tree(path: str) -> bool:
    """Return whether a cached path names Cargo's build directory.

    Parameters
    ----------
    path
        A single path as written in a cache step's ``path`` input.

    Returns
    -------
    bool
        ``True`` when the path is `target` or anything beneath it.
    """
    cleaned = path.strip().rstrip("/")
    return any(
        cleaned == root or cleaned.startswith(f"{root}/")
        for root in FORBIDDEN_CACHE_PATHS
    )


#: Every source of cache steps. Ubicloud lanes and GitHub-hosted lanes are
#: listed separately because their keys differ, not their provider.
UBICLOUD_CACHE_SOURCES = (
    (ACTION_DIR / "linux-gate-cache" / "action.yml", None),
    (ACTION_DIR / "kani-cache" / "action.yml", None),
    (WORKFLOW_DIR / "netsukefile-test.yml", "netsukefile"),
    (WORKFLOW_DIR / "coverage-main.yml", "coverage-upload"),
)
GITHUB_CACHE_SOURCES = ((ACTION_DIR / "windows-gate-cache" / "action.yml", None),)

#: Ubicloud lanes whose compiler cache belongs to `setup-rust`, mapped to the
#: `expect-cache` value each must pass. The action names the wrapper, starts
#: and zeroes the server, and selects the backend from the runner (ADR 0005 in
#: leynos/shared-actions): Ubicloud's cache proxy here, with the credentials
#: exported and the v2 switch cleared by the action itself. A lane with a fork
#: arm passes `any`, because a fork's run lands on a GitHub-hosted runner where
#: the action picks local disk; a lane that runs only on Ubicloud passes
#: `ubicloud`, so a missing proxy fails the job instead of compiling cold.
SETUP_RUST_SCCACHE_JOBS: dict[tuple[str, str], str] = {
    ("ci.yml", "build-test"): "any",
    ("netsukefile-test.yml", "netsukefile"): "any",
    ("coverage-main.yml", "coverage-upload"): "ubicloud",
}

#: Variables that configured the retired hand-rolled Linux cache. A caller's
#: value wins over `setup-rust`'s choice, so any of these left on a lane in
#: `SETUP_RUST_SCCACHE_JOBS` silently overrides the runner-aware backend.
HAND_ROLLED_SCCACHE_VARIABLES = (
    "RUSTC_WRAPPER",
    "SCCACHE_DIR",
    "SCCACHE_CACHE_SIZE",
    "SCCACHE_GHA_ENABLED",
)

#: GitHub-hosted Windows lanes use a workspace directory instead of the
#: GitHub Actions backend: that backend rate-limited every write there. They
#: therefore need no Actions cache credentials, and must not set the flag that
#: would re-enable the backend. They keep the hand-rolled local arm: their
#: gate caches own the Cargo paths (`cache-provider: external`), and
#: `setup-rust` owns a hosted runner's sccache directory only when it owns the
#: job's other caches too.
SCCACHE_LOCAL_DIR_JOBS = (
    ("ci-windows.yml", "lint-windows"),
    ("ci-windows.yml", "build-test-windows"),
    ("release.yml", "windows-native-recipe-smoke"),
)

#: No source build is permitted anywhere. The rule used to carry documented
#: exceptions, counted by name so a third could not arrive disguised as a
#: second occurrence of either; both have now been retired because their
#: crates began publishing binaries.
#:
#: `cargo-orthohelp` was permitted while leynos/ortho-config#479 left the crate
#: with no published binaries, and `mdtablefix` on Windows while
#: leynos/mdtablefix#458 left its binstall metadata broken and #459 left the
#: platform without an archive at all. ortho-config 0.9.1 and mdtablefix 0.5.1
#: publish checksum-verified archives for every platform this estate targets,
#: so both lanes now install prebuilt binaries.
#:
#: Kept as an empty mapping rather than deleted: it is the seam a future
#: exception would come back through, and an empty one states that the count
#: is zero rather than leaving the question unasked.
SOURCE_BUILD_EXCEPTIONS: dict[str, str] = {}

#: Tools that once had a source-build exception and must never regain one.
#: Checked by name rather than by a count of permitted builds, because a count
#: is satisfied by adding the tool back as a newly permitted entry, which is
#: precisely the regression worth catching.
FORBIDDEN_SOURCE_BUILDS = ("cargo-orthohelp", "mdtablefix")

#: Shared-action steps whose own cache would archive a `target` tree unless
#: the caller owns it. `rust-build-release` forwards the input to its nested
#: `setup-rust`, which is what keeps the packaging lane free of a build-tree
#: archive.
TARGET_ARCHIVE_OWNERS = (
    ("ci.yml", "build-test", "Setup Rust"),
    ("ci.yml", "build-test", "Test and Measure Coverage"),
    ("coverage-main.yml", "coverage-upload", "Setup Rust"),
    ("coverage-main.yml", "coverage-upload", "Test and Measure Coverage"),
    ("netsukefile-test.yml", "netsukefile", "Setup Rust"),
    ("build-and-package.yml", "build", "Build release binary"),
)

#: The one lane that compiles Rust without a compiler cache. On Windows sccache
#: re-spawns rustc with the aarch64 target's `--extern` and `-L` list and
#: exceeds the operating system's command-line limit, which nothing here can
#: shorten. Elsewhere the nested `setup-rust`, at the gate's pin, would select
#: the backend correctly, so the reason is cost: release builds are infrequent,
#: and a cache read that rarely would not pay back its setup.
SCCACHE_EXEMPT_LANE = ("build-and-package.yml", "build")

#: Jobs that compile Rust through a compiler cache they wire themselves: the
#: Windows lanes above. The Linux lanes reach theirs through `setup-rust`,
#: which `SETUP_RUST_SCCACHE_JOBS` holds.
SCCACHE_WRAPPER_JOBS = SCCACHE_LOCAL_DIR_JOBS

#: Every job whose `Setup Rust` step must delegate cache ownership to the
#: workflow. `setup-rust` archives `target/${BUILD_PROFILE}` and enables a
#: second sccache owner unless the caller says otherwise, so this inventory
#: lives beside the other tables rather than inline in one test: a lane added
#: to the placement tables but missed here would go unchecked in silence.
SETUP_RUST_DELEGATING_JOBS = (
    ("ci.yml", "build-test"),
    ("ci.yml", "kani-smoke"),
    ("ci-windows.yml", "lint-windows"),
    ("ci-windows.yml", "build-test-windows"),
    ("coverage-main.yml", "coverage-upload"),
    ("netsukefile-test.yml", "netsukefile"),
    ("release.yml", "windows-native-recipe-smoke"),
)

#: Steps of shared actions that carry their own cache and must be told to
#: delegate it. Distinct from `TARGET_ARCHIVE_OWNERS`, which covers the same
#: input on the actions that would archive a build tree; these are the
#: coverage and Whitaker payload archives.
DELEGATING_ACTION_STEPS = (
    ("ci.yml", "build-test", "Test and Measure Coverage"),
    ("coverage-main.yml", "coverage-upload", "Test and Measure Coverage"),
    ("ci.yml", "build-test", "Install Whitaker"),
    ("ci-windows.yml", "lint-windows", "Install Whitaker"),
)

#: The pinned `rust-build-release` revision, and the workflow that both names
#: it in a `uses:` reference and restates it in an environment variable so a
#: cache key can carry it. Held equal by the write-policy suite: the
#: `cargo-orthohelp` entry owns `~/.cargo/bin`, which also holds the
#: `cargo-binstall` that action provisions, so the entry must turn over when
#: the action's revision does.
RUST_BUILD_RELEASE_PIN_WORKFLOW = "build-and-package.yml"
RUST_BUILD_RELEASE_PIN_VARIABLE = "RUST_BUILD_RELEASE_PIN"
RUST_BUILD_RELEASE_ACTION = "leynos/shared-actions/.github/actions/rust-build-release@"

#: Jobs that call a cache action rather than declaring cache steps inline.
#: They must reference the composite for their platform and no other.
CACHE_ACTION_CALLERS = {
    ("ci.yml", "build-test"): "./.github/actions/linux-gate-cache",
    ("ci.yml", "kani-smoke"): "./.github/actions/kani-cache",
    ("ci-windows.yml", "lint-windows"): "./.github/actions/windows-gate-cache",
    ("ci-windows.yml", "build-test-windows"): "./.github/actions/windows-gate-cache",
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

#: The designated writers that declare their cache steps inline in a workflow
#: rather than delegating to a composite cache action. Derived from
#: `KEY_WRITERS` so a writer added there cannot escape the inline-save policy
#: check, and filtered by `CACHE_ACTION_CALLERS` so a delegating writer is not
#: searched for steps it does not declare.
INLINE_SAVE_WRITERS = tuple(
    writer for writer in KEY_WRITERS.values() if writer not in CACHE_ACTION_CALLERS
)

#: Jobs that call a cache composite with the read-only profile.
SMOKE_PROFILE_JOBS = (("release.yml", "windows-native-recipe-smoke"),)


def lane_steps(source: Path, job_name: str | None) -> list[dict[str, object]]:
    """Return the steps of a workflow job or of a composite action.

    Parameters
    ----------
    source
        Path to a workflow file or to a composite action's ``action.yml``.
    job_name
        Name of the job to read. Pass ``None`` to read a composite action
        instead, whose steps live under ``runs.steps`` rather than under a
        job. The two forms are one function because every caller quantifies
        over both kinds of lane with the same contract.

    Returns
    -------
    list[dict[str, object]]
        The lane's steps in declaration order. Order is part of the contract:
        several callers assert that a restore precedes an install, or that a
        statistics reset precedes the first compile.

    Notes
    -----
    A malformed document fails inside the ``require_*`` helpers rather than
    raising a typed error: ``job_name=None`` reads a composite action, whose
    steps live under ``runs.steps``, and a document that is not a mapping, or
    that is missing ``runs`` or ``steps``, or that holds a step that is not a
    mapping, is a contract failure rather than a harness error.
    """
    if job_name is not None:
        return job_steps(load_workflow(source), job_name)
    document = yaml.safe_load(source.read_text(encoding="utf-8"))
    runs = require_mapping(require_mapping(document, source.name).get("runs"), "runs")
    return [
        require_mapping(step, f"{source.name} step")
        for step in require_list(runs.get("steps"), f"{source.name} steps")
    ]


def cache_steps(steps: list[dict[str, object]]) -> list[dict[str, object]]:
    """Return every step that reserves, reads, or writes a cache archive.

    Parameters
    ----------
    steps
        A lane's steps, as returned by :func:`lane_steps`.

    Returns
    -------
    list[dict[str, object]]
        The subset whose ``uses`` names the cache action in any of its three
        spellings: combined, ``/restore``, or ``/save``. Matching the combined
        form matters, because a list built only from the split spellings would
        let every ownership contract pass over a real second owner in silence.
        Steps with no ``uses`` key, such as ``run`` steps, are excluded.
    """
    return [
        step for step in steps if CACHE_ACTION_PATTERN.search(str(step.get("uses", "")))
    ]


def declared_paths(step: dict[str, object]) -> list[str]:
    """Return the cached paths a cache step claims, one per line.

    Parameters
    ----------
    step
        A single cache step, as returned by :func:`cache_steps`.

    Returns
    -------
    list[str]
        The step's ``path`` input split on newlines, stripped, with blank
        lines dropped. A step declaring no ``path`` yields an empty list.

    Notes
    -----
    A step with no ``with`` mapping fails inside ``require_mapping``: a cache
    step claiming nothing would satisfy every ownership contract vacuously,
    which is the failure these contracts exist to catch.
    """
    inputs = require_mapping(step.get("with"), "cache step inputs")
    return [
        line.strip()
        for line in str(inputs.get("path", "")).splitlines()
        if line.strip()
    ]
