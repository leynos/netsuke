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

import dataclasses
import itertools
import os
import shutil
import sys
from pathlib import Path
from unittest import mock

import pytest
import yaml
from hypothesis import HealthCheck, given, settings
from hypothesis import strategies as st

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
    nested_dir: str,
    name: str,
    *,
    with_sidecar: bool = True,
) -> None:
    """Create an archive (and optionally its sidecar) in the nested layout.

    Parameters
    ----------
    dist
        Dist root to stage below.
    nested_dir
        Relative ``<artefact>/<staging-dir>`` directory to stage into,
        mirroring the two nesting levels the release workflow downloads.
    name
        Archive file name to create.
    with_sidecar
        Whether to create the matching ``.sha256`` sidecar.
    """
    nested = dist / nested_dir
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


def test_expected_names_reject_duplicate_target_triples(tmp_path: Path) -> None:
    """Two entries sharing a triple are rejected before anything moves.

    A repeated triple would derive the same archive name twice, so the same
    staged file would be claimed and moved twice; the second move would fail
    on the already-relocated source and roll back an otherwise valid release.
    """
    staging = tmp_path / "staging.toml"
    staging.write_text(
        '[targets.linux]\ntarget = "x86_64-unknown-linux-gnu"\n'
        '[targets.linux-again]\ntarget = "x86_64-unknown-linux-gnu"\n',
        encoding="utf-8",
    )
    manifest = tmp_path / "Cargo.toml"
    manifest.write_text('[package]\nname = "pkg"\n', encoding="utf-8")

    with pytest.raises(ValueError, match="duplicate release targets"):
        hoist_mod.expected_archive_names(staging, manifest, "1.0.0")


def test_hoist_moves_every_archive_and_sidecar_to_the_root(
    workspace: dict[str, Path],
) -> None:
    """The happy path relocates both files of every pair, preserving content."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64/s2", EXPECTED_NAMES[0])

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
            f"{artefact}/s",
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
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])

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
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(
        workspace["dist"],
        "netsuke-macos-arm64/s2",
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
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-linux-amd64-copy/s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64/s2", EXPECTED_NAMES[0])

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
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64/s2", EXPECTED_NAMES[0])
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
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64/s2", EXPECTED_NAMES[0])

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


def test_release_workflow_pins_the_hoist_interpreter() -> None:
    """The hoist must run under a pinned Python, not the runner's default.

    The script relies on `BaseExceptionGroup`, which needs Python 3.11 or
    later, so the release job installs the interpreter the repository's Python
    tooling targets rather than trusting whatever `python3` the runner ships.
    """
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    steps = workflow["jobs"]["release"]["steps"]
    hoist_index = next(
        index
        for index, step in enumerate(steps)
        if "hoist_binstall_archives.py" in step.get("run", "")
    )
    assert "--python 3.13" in steps[hoist_index]["run"], (
        "the hoist step must pin the interpreter version it runs under"
    )
    setup_index = next(
        index for index, step in enumerate(steps) if "setup-uv" in step.get("uses", "")
    )
    assert setup_index < hoist_index, (
        "the interpreter must be installed before the hoist step runs"
    )
    assert steps[setup_index]["with"]["python-version"] == "3.13", (
        "the installed interpreter must match the version the hoist step pins"
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


@pytest.mark.parametrize("colliding_suffix", ["", ".sha256"])
def test_hoist_rejects_a_directory_at_the_destination(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
    colliding_suffix: str,
) -> None:
    """A directory occupying either destination is a collision, not a target."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64/s2", EXPECTED_NAMES[0])
    blocking = workspace["dist"] / f"{EXPECTED_NAMES[0]}{colliding_suffix}"
    blocking.mkdir()

    assert run_hoist(workspace) == 1, (
        f"a directory at {blocking.name} must fail validation"
    )
    captured = capsys.readouterr()
    assert "destination already occupied" in captured.err, (
        "stderr must report the directory collision"
    )
    assert blocking.is_dir(), "the blocking directory must be left untouched"
    for nested_dir, name in (
        ("netsuke-linux-amd64/s1", EXPECTED_NAMES[1]),
        ("netsuke-macos-arm64/s2", EXPECTED_NAMES[0]),
    ):
        staged = workspace["dist"] / nested_dir / name
        assert staged.is_file(), (
            f"staged archive {name} must stay in place on a collision"
        )
    assert not (workspace["dist"] / EXPECTED_NAMES[1]).exists(), (
        "no valid pair may move while any destination is occupied"
    )


@pytest.mark.parametrize("linked_suffix", ["", ".sha256"])
def test_hoist_rejects_a_symlinked_staged_asset(
    workspace: dict[str, Path],
    capsys: pytest.CaptureFixture[str],
    linked_suffix: str,
) -> None:
    """A symlink standing in for either staged file fails validation.

    Moving a symlink would publish a broken link rather than the archive it
    points at, so both halves of the pair must be regular files.
    """
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    nested = workspace["dist"] / "netsuke-macos-arm64/s2"
    nested.mkdir(parents=True)
    real = workspace["dist"] / "real-payload"
    real.write_bytes(b"payload")
    linked_name = f"{EXPECTED_NAMES[0]}{linked_suffix}"
    (nested / linked_name).symlink_to(real)
    if linked_suffix:
        (nested / EXPECTED_NAMES[0]).write_bytes(b"archive")
    else:
        (nested / f"{EXPECTED_NAMES[0]}.sha256").write_text("sum", encoding="utf-8")

    assert run_hoist(workspace) == 1, f"a symlink at {linked_name} must fail validation"
    captured = capsys.readouterr()
    expected = "checksum sidecar absent" if linked_suffix else "not a regular file"
    assert expected in captured.err, (
        f"stderr must reject the symlinked asset; got {captured.err}"
    )
    assert_nothing_moved(workspace["dist"], EXPECTED_NAMES)


def test_hoist_rolls_back_completed_moves_when_a_move_fails(
    workspace: dict[str, Path],
) -> None:
    """A mid-phase move failure restores every file, and a rerun succeeds."""
    stage_pair(workspace["dist"], "netsuke-linux-amd64/s1", EXPECTED_NAMES[1])
    stage_pair(workspace["dist"], "netsuke-macos-arm64/s2", EXPECTED_NAMES[0])
    sources = {
        EXPECTED_NAMES[0]: workspace["dist"] / "netsuke-macos-arm64/s2",
        EXPECTED_NAMES[1]: workspace["dist"] / "netsuke-linux-amd64/s1",
    }

    real_move = shutil.move
    calls = {"count": 0}

    def failing_move(src, dst):  # noqa: ANN001, ANN202 - shutil.move signature
        calls["count"] += 1
        if calls["count"] == 3:
            msg = "injected move failure"
            raise OSError(msg)
        return real_move(src, dst)

    # The patch is scoped to the failing run so the retry below exercises the
    # real `shutil.move`, proving the rollback left a rerunnable tree.
    with (
        mock.patch.object(hoist_mod.shutil, "move", failing_move),
        pytest.raises(OSError, match="injected move failure"),
    ):
        run_hoist(workspace)

    for name, nested in sources.items():
        assert not (workspace["dist"] / name).exists(), (
            f"{name} must not remain at the release root after rollback"
        )
        assert not (workspace["dist"] / f"{name}.sha256").exists(), (
            f"{name}.sha256 must not remain at the release root after rollback"
        )
        restored = nested / name
        assert restored.is_file(), f"{name} must return to its nested path"
        assert restored.read_bytes() == f"archive:{name}".encode(), (
            f"{name} must keep its original content through the rollback"
        )
        sidecar = nested / f"{name}.sha256"
        assert sidecar.is_file(), f"{name}.sha256 must return to its nested path"
        assert sidecar.read_text(encoding="utf-8") == f"checksum:{name}", (
            f"{name}.sha256 must keep its original content through the rollback"
        )

    assert run_hoist(workspace) == 0, "a rerun after the fault clears must succeed"
    for name in EXPECTED_NAMES:
        assert (workspace["dist"] / name).is_file(), (
            f"{name} must reach the release root on the retry"
        )
        assert (workspace["dist"] / name).read_bytes() == (
            f"archive:{name}".encode()
        ), f"{name} content must survive the retry"


TARGET_POOL = [
    "x86_64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "riscv64gc-unknown-linux-gnu",
]
STATE_POOL = ["ok", "missing-archive", "missing-sidecar", "duplicate", "collision"]


@dataclasses.dataclass(frozen=True)
class GeneratedHoistCase:
    """One generated hoist scenario for the property test.

    Parameters
    ----------
    package
        Generated Cargo package name.
    version
        Generated semantic-style release version.
    targets_and_states
        Unique target triples paired with the staging state to apply.
    layout_seed
        Seed varying the generated nested staging-directory layout.
    """

    package: str
    version: str
    targets_and_states: tuple[tuple[str, str], ...]
    layout_seed: int

    def archive_name(self, target: str) -> str:
        """Return the expected archive name for ``target``.

        Parameters
        ----------
        target
            Target triple to interpolate.

        Returns
        -------
        str
            The ``{package}-{version}-{target}.tar.gz`` archive name.
        """
        return f"{self.package}-{self.version}-{target}.tar.gz"

    @property
    def expects_failure(self) -> bool:
        """Whether any target is in a state that must fail validation."""
        return any(state != "ok" for _target, state in self.targets_and_states)


GENERATED_CASES = st.builds(
    GeneratedHoistCase,
    package=st.from_regex(r"[a-z][a-z0-9-]{0,12}", fullmatch=True),
    version=st.from_regex(r"[0-9]{1,2}\.[0-9]{1,2}\.[0-9]{1,2}", fullmatch=True),
    targets_and_states=st.lists(
        st.tuples(st.sampled_from(TARGET_POOL), st.sampled_from(STATE_POOL)),
        min_size=1,
        max_size=4,
        unique_by=lambda pair: pair[0],
    ).map(tuple),
    layout_seed=st.integers(min_value=0, max_value=7),
)


def build_generated_workspace(root: Path, case: GeneratedHoistCase) -> dict[str, Path]:
    """Write the generated staging and manifest TOML and create ``dist``.

    Parameters
    ----------
    root
        Fresh directory to build the workspace in.
    case
        Generated scenario supplying the targets and package name.

    Returns
    -------
    dict[str, Path]
        Paths keyed by ``staging``, ``manifest``, and ``dist``.
    """
    staging = root / "staging.toml"
    staging.write_text(
        "".join(
            f'[targets.t{index}]\ntarget = "{target}"\n'
            for index, (target, _state) in enumerate(case.targets_and_states)
        ),
        encoding="utf-8",
    )
    manifest = root / "Cargo.toml"
    manifest.write_text(f'[package]\nname = "{case.package}"\n', encoding="utf-8")
    dist = root / "dist"
    dist.mkdir()
    return {"staging": staging, "manifest": manifest, "dist": dist}


def stage_generated_target(
    dist: Path, case: GeneratedHoistCase, index: int
) -> list[Path]:
    """Stage one target in its generated state below ``dist``.

    Parameters
    ----------
    dist
        Dist root to stage below.
    case
        Generated scenario supplying the name, layout seed, and state.
    index
        Position of the target within ``case.targets_and_states``.

    Returns
    -------
    list[Path]
        The staged paths that must remain unchanged when validation fails.
    """
    target, state = case.targets_and_states[index]
    name = case.archive_name(target)
    staged: list[Path] = []
    nested = dist / f"artefact-{index}" / f"stage-{(index + case.layout_seed) % 3}"
    if state != "missing-archive":
        nested.mkdir(parents=True, exist_ok=True)
        (nested / name).write_bytes(f"archive:{name}".encode())
        staged.append(nested / name)
        if state != "missing-sidecar":
            (nested / f"{name}.sha256").write_text("c", encoding="utf-8")
            staged.append(nested / f"{name}.sha256")
    if state == "duplicate":
        other = dist / f"artefact-{index}-dup" / "stage"
        other.mkdir(parents=True)
        (other / name).write_bytes(b"dup")
        staged.append(other / name)
    if state == "collision":
        (dist / name).write_bytes(b"occupied")
    return staged


def assert_invalid_generated_outcome(
    dist: Path,
    case: GeneratedHoistCase,
    staged_paths: list[Path],
    status: int,
) -> None:
    """Assert an invalid generated case failed before moving anything.

    Parameters
    ----------
    dist
        Dist root the hoist ran against.
    case
        Generated scenario supplying the collision names.
    staged_paths
        Every staged path that must remain unchanged.
    status
        Exit status returned by the hoist.
    """
    assert status == 1, "any invalid target state must fail validation"
    for staged in staged_paths:
        assert staged.exists(), (
            f"invalid sets must leave staged file {staged} unchanged"
        )
    for target, state in case.targets_and_states:
        if state == "collision":
            occupied = dist / case.archive_name(target)
            assert occupied.read_bytes() == b"occupied", (
                "pre-existing destination entries must be untouched"
            )


def assert_valid_generated_outcome(
    dist: Path, case: GeneratedHoistCase, status: int
) -> None:
    """Assert a valid generated case hoisted every derived pair.

    Parameters
    ----------
    dist
        Dist root the hoist ran against.
    case
        Generated scenario the archive names derive from.
    status
        Exit status returned by the hoist.
    """
    assert status == 0, "a complete, unique, collision-free set must succeed"
    for target, _state in case.targets_and_states:
        name = case.archive_name(target)
        assert (dist / name).is_file(), (
            f"{name} must be derived from the generated inputs and hoisted"
        )
        assert (dist / f"{name}.sha256").is_file(), (
            f"{name}.sha256 must accompany its archive at the root"
        )


@settings(
    max_examples=25,
    deadline=None,
    derandomize=True,
    suppress_health_check=[HealthCheck.function_scoped_fixture],
)
@given(case=GENERATED_CASES)
def test_hoist_invariants_hold_for_generated_layouts(
    tmp_path_factory: pytest.TempPathFactory,
    case: GeneratedHoistCase,
) -> None:
    """All-or-none and name-derivation invariants hold across generated cases."""
    workspace = build_generated_workspace(
        tmp_path_factory.mktemp("hoist-property"), case
    )
    staged_paths = [
        path
        for index in range(len(case.targets_and_states))
        for path in stage_generated_target(workspace["dist"], case, index)
    ]

    status = hoist_mod.hoist(
        workspace["dist"], workspace["staging"], workspace["manifest"], case.version
    )

    if case.expects_failure:
        assert_invalid_generated_outcome(workspace["dist"], case, staged_paths, status)
    else:
        assert_valid_generated_outcome(workspace["dist"], case, status)
