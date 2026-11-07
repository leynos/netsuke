"""Test the selection CLI end to end over a real staged tree.

Each case builds the tree a release downloads -- one directory per producer,
hoisted archives at the root, and the build products that ship inside the
packages -- and asserts on what the CLI left behind. Asserting on the exit code
alone would pass for a run that refused everything, so every case also states
whether a payload exists and which names it holds.

Run via ``make test-release-publication``.
"""

import dataclasses
import json
import typing as typ

import pytest
import release_digest_files
import release_digest_record_files
import select_release_payload_cli as cli

if typ.TYPE_CHECKING:
    import pathlib

CANDIDATE = "a" * 40
RUN_ID = "1234"
VERSION = "1.0.0"
ARCHIVE = f"netsuke-{VERSION}-x86_64-unknown-linux-gnu.tar.gz"
SIDECAR = f"{ARCHIVE}.sha256"
DEB = f"netsuke_{VERSION}_amd64.deb"
RPM = f"netsuke-{VERSION}-1.x86_64.rpm"
TARGET = "linux-x86_64"

#: The smallest staging configuration that still exercises the platform count:
#: one Linux target must stage exactly one ``.deb`` and one ``.rpm``.
STAGING_CONFIG = """
[common]
bin_name = "netsuke"

[targets.linux-x86_64]
target = "x86_64-unknown-linux-gnu"
platform = "linux"
"""

MANIFEST = """
[package]
name = "netsuke"
version = "1.0.0"
"""

#: Files a real download carries that no release publishes. Selection must
#: exclude them rather than refusing a perfectly good build.
RETAINED = ("netsuke", "LICENSE", "completions/bash/netsuke")


def stage(dist: pathlib.Path, relative: str, content: bytes = b"bytes") -> bytes:
    """Write ``content`` below ``dist`` and return it.

    Returns
    -------
    bytes
        The content written.
    """
    path = dist / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    _ = path.write_bytes(content)
    return content


@dataclasses.dataclass(frozen=True, slots=True)
class Provenance:
    """Name the fields a case varies in the producer record it stages.

    Attributes
    ----------
    commit
        Candidate the producer checked out.
    run_id
        Workflow run the producer belongs to.
    platform
        Operating system the target package is built for.
    target
        Target name the record is filed under.
    """

    commit: str = CANDIDATE
    run_id: str = RUN_ID
    platform: str = "linux"
    target: str = TARGET


def record_provenance(
    dist: pathlib.Path,
    *,
    fields: Provenance | None = None,
    assets: dict[str, str] | None = None,
) -> None:
    """Write a producer record below ``dist`` the way a producer job would."""
    fields = fields or Provenance()
    payload = {
        "schema": 1,
        "commit": fields.commit,
        "run_id": fields.run_id,
        "run_attempt": "1",
        "target": fields.target,
        "platform": fields.platform,
        "assets": assets if assets is not None else {},
    }
    path = dist / fields.target / "producer-provenance.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    _ = path.write_text(json.dumps(payload), encoding="utf-8")


def staged_tree(tmp_path: pathlib.Path, *, producer: bool = True) -> dict[str, object]:
    """Build a complete, publishable staged tree and its inputs.

    Returns
    -------
    dict[str, object]
        The keyword arguments :func:`run_cli` needs, plus the staged bytes.
    """
    dist = tmp_path / "dist"
    for name in (ARCHIVE, SIDECAR, DEB, RPM):
        stage(dist, name, name.encode("utf-8"))
    for retained in RETAINED:
        stage(dist, retained)
    if producer:
        record_provenance(dist, assets=_staged_assets({"dist": dist}))
    staging_config = tmp_path / "release-staging.toml"
    _ = staging_config.write_text(STAGING_CONFIG, encoding="utf-8")
    manifest = tmp_path / "Cargo.toml"
    _ = manifest.write_text(MANIFEST, encoding="utf-8")
    return {
        "dist": dist,
        "staging_config": staging_config,
        "manifest": manifest,
        "output": tmp_path / "artifacts",
        "record": tmp_path / "artifacts" / "trusted-record.json",
    }


def run_cli(tree: dict[str, object], **overrides: object) -> int:
    """Invoke the CLI over ``tree``, overriding any argument by name.

    Returns
    -------
    int
        The CLI's exit status.
    """
    arguments: dict[str, object] = {
        "dist": tree["dist"],
        "staging_config": tree["staging_config"],
        "manifest": tree["manifest"],
        "output": tree["output"],
        "record": tree["record"],
    }
    arguments |= overrides
    return cli.main([
        "--dist-dir",
        str(arguments["dist"]),
        "--staging-config",
        str(arguments["staging_config"]),
        "--manifest",
        str(arguments["manifest"]),
        "--version",
        VERSION,
        "--commit",
        str(arguments.get("commit", CANDIDATE)),
        "--run-id",
        str(arguments.get("run_id", RUN_ID)),
        "--output-dir",
        str(arguments["output"]),
        "--digest-record",
        str(arguments["record"]),
    ])


def installed_names(output: pathlib.Path) -> list[str]:
    """Return the names the CLI copied into ``output``, sorted."""
    payload = output / cli.PAYLOAD_DIRECTORY
    return sorted(path.name for path in payload.iterdir())


def test_a_complete_tree_installs_exactly_the_declared_names(
    tmp_path: pathlib.Path,
) -> None:
    """Copy the declared assets under their published names and nothing else."""
    tree = staged_tree(tmp_path)
    assert run_cli(tree) == cli.EXIT_OK, "a complete tree must be accepted"
    assert installed_names(tree["output"]) == sorted([ARCHIVE, SIDECAR, DEB, RPM]), (
        "the payload must hold exactly the declared names"
    )


def test_the_trusted_record_describes_the_installed_bytes(
    tmp_path: pathlib.Path,
) -> None:
    """Write a record the verifier and finalizer can compare downloads against."""
    tree = staged_tree(tmp_path)
    assert run_cli(tree) == cli.EXIT_OK, "a complete tree must be accepted"
    record = release_digest_record_files.read_digest_record(tree["record"])
    assert sorted(record) == sorted([ARCHIVE, SIDECAR, DEB, RPM]), (
        "the record must describe every declared name"
    )
    installed = tree["output"] / cli.PAYLOAD_DIRECTORY / DEB
    assert record[DEB] == release_digest_files.digest_of_file(installed), (
        "the recorded digest must describe the installed bytes"
    )


def test_retained_material_never_reaches_the_payload(tmp_path: pathlib.Path) -> None:
    """Exclude the binary, licence, and completions the packages contain.

    Refusing them would reject a good build; uploading them would publish files
    the release never declared.
    """
    tree = staged_tree(tmp_path)
    assert run_cli(tree) == cli.EXIT_OK, "retained material must not be refused"
    recorded = release_digest_record_files.read_digest_record(tree["record"])
    for retained in RETAINED:
        assert retained not in installed_names(tree["output"]), retained
        assert retained not in recorded, retained


def test_the_producer_record_is_not_uploaded_as_an_asset(
    tmp_path: pathlib.Path,
) -> None:
    """Leave the provenance record out of the payload it describes."""
    tree = staged_tree(tmp_path)
    assert run_cli(tree) == cli.EXIT_OK, "the record must not be an asset"
    assert "producer-provenance.json" not in installed_names(tree["output"]), (
        "the producer record must stay out of the payload"
    )


def test_a_producer_from_another_commit_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse a tree left over from an earlier candidate."""
    tree = staged_tree(tmp_path, producer=False)
    record_provenance(
        tree["dist"],
        fields=Provenance(commit="b" * 40),
        assets=_staged_assets(tree),
    )
    assert run_cli(tree) == cli.EXIT_PAYLOAD_REFUSED, (
        "a stale candidate must be refused"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_a_producer_from_another_run_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse a record whose producer belongs to a different workflow run."""
    tree = staged_tree(tmp_path, producer=False)
    record_provenance(
        tree["dist"], fields=Provenance(run_id="9999"), assets=_staged_assets(tree)
    )
    assert run_cli(tree) == cli.EXIT_PAYLOAD_REFUSED, "a foreign run must be refused"
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_a_record_naming_an_unknown_platform_is_unusable_input(
    tmp_path: pathlib.Path,
) -> None:
    """Report a broken record as input trouble, not as a refused payload.

    Re-running publication cannot repair it; the producer job must be fixed.
    """
    tree = staged_tree(tmp_path, producer=False)
    record_provenance(
        tree["dist"], fields=Provenance(platform="beos"), assets=_staged_assets(tree)
    )
    assert run_cli(tree) == cli.EXIT_UNUSABLE_INPUT, (
        "an unknown platform is input trouble, not a refusal"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_an_ambiguous_declared_name_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse two staged files claiming one published name."""
    tree = staged_tree(tmp_path)
    stage(tree["dist"], f"stray/{DEB}")
    assert run_cli(tree) == cli.EXIT_PAYLOAD_REFUSED, (
        "an ambiguous name must be refused"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_a_release_shaped_stray_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse an undeclared staged file that would publish if uploaded."""
    tree = staged_tree(tmp_path)
    stage(tree["dist"], "netsuke-9.9.9-aarch64-apple-darwin.tar.gz")
    assert run_cli(tree) == cli.EXIT_PAYLOAD_REFUSED, (
        "a release-shaped stray must be refused"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_a_missing_declared_asset_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse a tree that lost one of the recorded assets."""
    tree = staged_tree(tmp_path)
    _ = (tree["dist"] / RPM).unlink()
    assert run_cli(tree) == cli.EXIT_PAYLOAD_REFUSED, (
        "a lost declared asset must be refused"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_replaced_bytes_are_refused_before_the_copy(tmp_path: pathlib.Path) -> None:
    """Refuse a staged file whose bytes the producer did not record."""
    tree = staged_tree(tmp_path, producer=False)
    record_provenance(tree["dist"], assets=_staged_assets(tree))
    stage(tree["dist"], DEB, b"rebuilt from another commit")
    assert run_cli(tree) == cli.EXIT_PAYLOAD_REFUSED, (
        "bytes the producer did not record must be refused"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_a_missing_producer_record_is_refused(tmp_path: pathlib.Path) -> None:
    """Refuse a tree whose producers never reported, naming the target.

    The download lost a producer's record, so the tree is not this release's
    even though every file in it is readable.
    """
    tree = staged_tree(tmp_path, producer=False)
    assert run_cli(tree) == cli.EXIT_PAYLOAD_REFUSED, (
        "a producer that never reported must be refused"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def test_an_existing_output_directory_is_unusable_input(
    tmp_path: pathlib.Path,
) -> None:
    """Refuse to merge into a directory left by an earlier attempt."""
    tree = staged_tree(tmp_path)
    tree["output"].mkdir()
    assert run_cli(tree) == cli.EXIT_UNUSABLE_INPUT, (
        "an existing output directory must be refused"
    )


def test_a_malformed_staging_config_is_unusable_input(
    tmp_path: pathlib.Path,
) -> None:
    """Report a configuration that cannot declare any release."""
    tree = staged_tree(tmp_path)
    _ = tree["staging_config"].write_text("not = [toml", encoding="utf-8")
    assert run_cli(tree) == cli.EXIT_UNUSABLE_INPUT, (
        "a malformed config must be reported as input trouble"
    )
    assert not tree["output"].exists(), "a refusal must not leave a payload"


def _staged_assets(tree: dict[str, object]) -> dict[str, str]:
    """Return the published name to digest mapping the tree stages.

    Returns
    -------
    dict[str, str]
        Each declared name and the digest of its staged bytes.
    """
    dist = tree["dist"]
    return {
        name: release_digest_files.digest_of_file(dist / name)
        for name in (ARCHIVE, SIDECAR, DEB, RPM)
    }


@pytest.mark.parametrize("name", [ARCHIVE, SIDECAR, DEB, RPM])
def test_every_declared_name_is_copied_from_its_own_staged_file(
    tmp_path: pathlib.Path, *, name: str
) -> None:
    """Copy each name's bytes, not one asset's bytes under every name."""
    tree = staged_tree(tmp_path)
    assert run_cli(tree) == cli.EXIT_OK, "a complete tree must be accepted"
    installed = tree["output"] / cli.PAYLOAD_DIRECTORY / name
    assert installed.read_bytes() == name.encode("utf-8"), (
        "each name must be copied from its own staged file"
    )
