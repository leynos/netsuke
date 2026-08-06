"""Behavioural and wiring tests for the cargo-binstall hoist step.

``scripts/hoist_binstall_archives.py`` moves the staged
``{package}-{version}-{target}.tar.gz`` archives (and their ``.sha256``
sidecars) from the downloaded workflow-artefact layout to the ``dist/`` root,
where ``upload-release-assets`` publishes them under the plain names the
binstall metadata in ``Cargo.toml`` resolves. These tests stage representative
layouts and pin the load-bearing behaviour: every expected target must be
present with its checksum before anything moves, every validation failure
leaves the tree untouched (all-or-none), and a shortfall fails with the
missing names listed rather than uploading a partial asset set.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import itertools
import os
import sys
from pathlib import Path

import pytest
import yaml

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts"))

# The module under test lives in scripts/, outside any package; the import
# must follow the sys.path insertion above, hence the E402 suppression.
import hoist_binstall_archives as hoist_mod  # noqa: E402

WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"

STAGING_CONFIG = """\
[common]
bin_name = "netsuke"

[common.binstall]
enabled = true

[targets.linux-x86_64]
target = "x86_64-unknown-linux-gnu"

[targets.macos-aarch64]
target = "aarch64-apple-darwin"
"""

MANIFEST = """\
[package]
name = "netsuke-build"
version = "9.9.9"
"""

VERSION = "9.9.9"
EXPECTED_NAMES = [
    "netsuke-build-9.9.9-aarch64-apple-darwin.tar.gz",
    "netsuke-build-9.9.9-x86_64-unknown-linux-gnu.tar.gz",
]


@pytest.fixture
def workspace(tmp_path: Path) -> dict[str, Path]:
    """Provide a staging config, manifest, and empty dist directory.

    Parameters
    ----------
    tmp_path
        pytest-provided temporary directory.

    Returns
    -------
    dict[str, Path]
        Paths keyed by ``staging``, ``manifest``, and ``dist``.
    """
    staging = tmp_path / "release-staging.toml"
    staging.write_text(STAGING_CONFIG, encoding="utf-8")
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text(MANIFEST, encoding="utf-8")
    dist = tmp_path / "dist"
    dist.mkdir()
    return {"staging": staging, "manifest": manifest, "dist": dist}


def stage_pair(
    dist: Path,
    artefact: str,
    staging_dir: str,
    name: str,
    *,
    with_sidecar: bool = True,
) -> None:
    """Create an archive (and optionally its sidecar) in the nested layout.

    Parameters
    ----------
    dist
        Dist root to stage below.
    artefact
        Workflow-artefact directory name (first nesting level).
    staging_dir
        Staging directory name (second nesting level).
    name
        Archive file name to create.
    with_sidecar
        Whether to create the matching ``.sha256`` sidecar.
    """
    nested = dist / artefact / staging_dir
    nested.mkdir(parents=True, exist_ok=True)
    (nested / name).write_bytes(f"archive:{name}".encode())
    if with_sidecar:
        (nested / f"{name}.sha256").write_text(f"checksum:{name}", encoding="utf-8")


def run_hoist(workspace: dict[str, Path]) -> int:
    """Invoke the hoist against the fixture workspace.

    Parameters
    ----------
    workspace
        Fixture mapping from the :func:`workspace` fixture.

    Returns
    -------
    int
        The hoist's exit status.
    """
    return hoist_mod.hoist(
        workspace["dist"], workspace["staging"], workspace["manifest"], VERSION
    )


def assert_nothing_moved(dist: Path, names: list[str]) -> None:
    """Assert no expected asset reached the dist root.

    Parameters
    ----------
    dist
        Dist root to inspect.
    names
        Expected archive names that must not appear at the root.
    """
    for name in names:
        assert not (dist / name).exists(), (
            f"{name} must not reach the dist root on a validation failure"
        )
        assert not (dist / f"{name}.sha256").exists(), (
            f"{name}.sha256 must not reach the dist root on a validation failure"
        )


def test_expected_names_derive_from_staging_and_manifest(
    workspace: dict[str, Path],
) -> None:
    """The expected archive set comes from the config files, not literals."""
    names = hoist_mod.expected_archive_names(
        workspace["staging"], workspace["manifest"], VERSION
    )
    assert names == EXPECTED_NAMES, (
        f"expected names should derive from staging config and manifest; got {names}"
    )


def test_expected_names_track_alternative_inputs(tmp_path: Path) -> None:
    """Different package names, versions, and targets flow into the names."""
    staging = tmp_path / "staging.toml"
    staging.write_text(
        '[targets.only]\ntarget = "riscv64gc-unknown-linux-gnu"\n',
        encoding="utf-8",
    )
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text('[package]\nname = "otherpkg"\n', encoding="utf-8")

    names = hoist_mod.expected_archive_names(staging, manifest, "3.2.1")
    assert names == ["otherpkg-3.2.1-riscv64gc-unknown-linux-gnu.tar.gz"], (
        f"names must interpolate package, version, and target; got {names}"
    )


def test_expected_names_reject_an_empty_target_table(tmp_path: Path) -> None:
    """A staging config without targets is a configuration error."""
    staging = tmp_path / "staging.toml"
    staging.write_text("[targets]\n", encoding="utf-8")
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text('[package]\nname = "pkg"\n', encoding="utf-8")

    with pytest.raises(ValueError, match="defines no release targets"):
        hoist_mod.expected_archive_names(staging, manifest, "1.0.0")


def test_hoist_moves_every_archive_and_sidecar_to_the_root(
    workspace: dict[str, Path],
) -> None:
    """The happy path relocates both files of every pair, preserving content."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64", "s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64", "s2", EXPECTED_NAMES[0])

    assert run_hoist(workspace) == 0, "hoist should succeed when all pairs staged"
    for name in EXPECTED_NAMES:
        moved = workspace["dist"] / name
        assert moved.is_file(), f"{name} should sit at the dist root after hoisting"
        assert moved.read_bytes() == f"archive:{name}".encode(), (
            f"{name} content must survive the move"
        )
        sidecar = workspace["dist"] / f"{name}.sha256"
        assert sidecar.is_file(), f"{name}.sha256 should sit at the dist root"
        assert sidecar.read_text(encoding="utf-8") == f"checksum:{name}", (
            f"{name}.sha256 content must survive the move"
        )
    leftovers = [
        path
        for path in workspace["dist"].rglob("*.tar.gz*")
        if path.parent != workspace["dist"]
    ]
    assert leftovers == [], f"no staged copies should remain nested: {leftovers}"


@pytest.mark.parametrize(
    ("first_state", "second_state"),
    [
        pair
        for pair in itertools.product(
            ["ok", "missing-archive", "missing-sidecar"], repeat=2
        )
        if pair != ("ok", "ok")
    ],
)
def test_hoist_moves_nothing_for_every_failing_state_combination(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
    first_state: str,
    second_state: str,
) -> None:
    """Exhaustively: any target in a failing state means nothing moves."""
    for name, state, artefact in (
        (EXPECTED_NAMES[0], first_state, "a1"),
        (EXPECTED_NAMES[1], second_state, "a2"),
    ):
        if state == "missing-archive":
            continue
        stage_pair(
            workspace["dist"],
            artefact,
            "s",
            name,
            with_sidecar=state != "missing-sidecar",
        )

    assert run_hoist(workspace) == 1, (
        f"states ({first_state}, {second_state}) must fail validation"
    )
    assert_nothing_moved(workspace["dist"], EXPECTED_NAMES)
    captured = capsys.readouterr()
    assert "Missing cargo-binstall release assets" in captured.err, (
        "the failure must list the missing assets on stderr"
    )


def test_hoist_failure_names_the_missing_target(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A missing target's archive name appears verbatim in the error."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64", "s1", EXPECTED_NAMES[1])

    assert run_hoist(workspace) == 1, "a missing target must fail the hoist"
    captured = capsys.readouterr()
    assert EXPECTED_NAMES[0] in captured.err, (
        f"stderr must name the missing archive {EXPECTED_NAMES[0]}"
    )
    nested = workspace["dist"] / "netsuke-linux-amd64" / "s1" / EXPECTED_NAMES[1]
    assert nested.is_file(), "the staged archive must stay in place on failure"


def test_hoist_fails_when_a_checksum_sidecar_is_absent(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
) -> None:
    """An archive without its .sha256 sidecar fails validation untouched."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64", "s1", EXPECTED_NAMES[1])
    stage_pair(
        workspace["dist"],
        "netsuke-macos-arm64",
        "s2",
        EXPECTED_NAMES[0],
        with_sidecar=False,
    )

    assert run_hoist(workspace) == 1, "a missing sidecar must fail the hoist"
    captured = capsys.readouterr()
    assert f"{EXPECTED_NAMES[0]}.sha256" in captured.err, (
        "stderr must name the absent checksum sidecar"
    )
    assert_nothing_moved(workspace["dist"], EXPECTED_NAMES)


def test_hoist_rejects_duplicate_archive_candidates(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Two nested copies of one archive are ambiguous; nothing moves."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64", "s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-linux-amd64-copy", "s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64", "s2", EXPECTED_NAMES[0])

    assert run_hoist(workspace) == 1, "duplicate candidates must fail the hoist"
    captured = capsys.readouterr()
    assert "found 2 candidates" in captured.err, (
        "stderr must report the ambiguous candidate count"
    )
    assert_nothing_moved(workspace["dist"], EXPECTED_NAMES)


def test_hoist_rejects_an_occupied_destination(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A same-named file already at the dist root blocks the move."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64", "s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64", "s2", EXPECTED_NAMES[0])
    (workspace["dist"] / EXPECTED_NAMES[0]).write_bytes(b"impostor")

    assert run_hoist(workspace) == 1, "a destination collision must fail the hoist"
    captured = capsys.readouterr()
    assert "destination already occupied" in captured.err, (
        "stderr must report the destination collision"
    )
    assert (workspace["dist"] / EXPECTED_NAMES[0]).read_bytes() == b"impostor", (
        "the pre-existing root file must not be overwritten"
    )
    staged = workspace["dist"] / "netsuke-linux-amd64" / "s1" / EXPECTED_NAMES[1]
    assert staged.is_file(), "staged archives must stay in place on collision"


@pytest.mark.skipif(os.geteuid() == 0, reason="root bypasses permission bits")
def test_hoist_propagates_traversal_errors(
    workspace: dict[str, Path],
) -> None:
    """An unreadable directory surfaces as an error, not a missing asset."""
    blocked = workspace["dist"] / "blocked"
    blocked.mkdir()
    blocked.chmod(0o000)
    try:
        with pytest.raises(PermissionError):
            run_hoist(workspace)
    finally:
        blocked.chmod(0o755)


def test_main_runs_the_hoist_from_cli_arguments(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
) -> None:
    """The CLI entry point used by release.yml wires arguments to the hoist."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64", "s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64", "s2", EXPECTED_NAMES[0])

    status = hoist_mod.main(
        [
            "--version",
            VERSION,
            "--dist-dir",
            str(workspace["dist"]),
            "--staging-config",
            str(workspace["staging"]),
            "--manifest",
            str(workspace["manifest"]),
        ]
    )
    assert status == 0, "the CLI must exit 0 on a fully staged dist"
    captured = capsys.readouterr()
    assert "Hoisted 2 archive/sidecar pairs" in captured.out, (
        "the CLI must report the hoisted pair count"
    )
    for name in EXPECTED_NAMES:
        assert (workspace["dist"] / name).is_file(), (
            f"the CLI invocation must move {name} to the dist root"
        )


def test_release_workflow_invokes_the_hoist_script() -> None:
    """The release job must run the script with the resolved version."""
    workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
    assert "scripts/hoist_binstall_archives.py" in workflow, (
        "release.yml must invoke the hoist script"
    )
    assert "--version '${{ needs.metadata.outputs.version }}'" in workflow, (
        "release.yml must pass the resolved release version to the script"
    )


def test_release_workflow_hoists_before_uploading() -> None:
    """The hoist step must precede the asset upload in the release job."""
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    steps = workflow["jobs"]["release"]["steps"]
    hoist_index = next(
        index
        for index, step in enumerate(steps)
        if "hoist_binstall_archives.py" in step.get("run", "")
    )
    upload_index = next(
        index for index, step in enumerate(steps) if step.get("id") == "upload_assets"
    )
    assert hoist_index < upload_index, (
        "the hoist must run before upload_assets so only validated, hoisted "
        "archives are published"
    )
