"""Exercise the declared-name CLI the publication jobs invoke.

The declaration itself is proved in :mod:`test_release_declaration`. These
tests pin the seam the workflow depends on: that the written file is the format
the inventory verifier reads, that the written digest record is the one the
verifier compares against, that a refused declaration leaves neither file
behind, and that an unreadable input is reported separately from a refused
release. Nothing here opens a socket.
"""

import typing as typ

import pytest
import release_digest_files
import release_digest_record_files
import release_inventory
from release_declaration_cli import (
    EXIT_INPUT_UNUSABLE,
    EXIT_MISSING_ASSETS,
    EXIT_OK,
    build_parser,
    main,
)
from release_staging_support import ARCHIVE, SIDECAR
from release_staging_support import stage_archive as _stage_archive
from verify_release_inventory_cli import verify

if typ.TYPE_CHECKING:
    from pathlib import Path

STAGING_CONFIG = """\
[targets.linux-x86_64]
platform = "linux"
target = "x86_64-unknown-linux-gnu"
"""

MANIFEST = """\
[package]
name = "netsuke-build"
version = "0.1.0"
"""


def _workspace(tmp_path: Path, *, stage_packages: bool = True) -> dict[str, Path]:
    """Write the inputs the CLI reads and the tree it inspects.

    The archive and its sidecar are staged whenever the packages are, because
    the declaration names them from configuration rather than from the tree. A
    tree holding no archive cannot yield the digest record the verifier reads,
    so a fixture that declared without staging one would be asserting against a
    release the producer refuses.

    Returns
    -------
    dict[str, Path]
        Paths keyed by ``staging``, ``manifest``, ``dist``, ``output``, and
        ``record``.
    """
    staging = tmp_path / "release-staging.toml"
    _ = staging.write_text(STAGING_CONFIG, encoding="utf-8")
    manifest = tmp_path / "Cargo.toml"
    _ = manifest.write_text(MANIFEST, encoding="utf-8")
    dist = tmp_path / "dist"
    dist.mkdir()
    _stage_archive(dist)
    if stage_packages:
        for name in ("netsuke-0.1.0-1.x86_64.rpm", "netsuke_0.1.0-1_amd64.deb"):
            _ = (dist / name).write_bytes(f"package:{name}".encode())
    return {
        "staging": staging,
        "manifest": manifest,
        "dist": dist,
        "output": tmp_path / "declared.txt",
        "record": tmp_path / "trusted.json",
    }


def _argv(workspace: dict[str, Path]) -> list[str]:
    """Return a producer invocation naming every required argument."""
    return [
        "--dist-dir",
        str(workspace["dist"]),
        "--staging-config",
        str(workspace["staging"]),
        "--manifest",
        str(workspace["manifest"]),
        "--version",
        "0.1.0",
        "--output",
        str(workspace["output"]),
        "--digest-record",
        str(workspace["record"]),
    ]


def test_the_cli_writes_the_declaration_the_verifier_reads(tmp_path: Path) -> None:
    """Produce a file the inventory verifier accepts without translation.

    The two modules are separate on purpose, so the round trip is what proves
    they still agree about the format. A drift here would be invisible until a
    release ran.
    """
    workspace = _workspace(tmp_path)

    code = main(_argv(workspace))

    assert code == EXIT_OK, f"a fully staged release must declare, got {code}"
    names = workspace["output"].read_text(encoding="utf-8").splitlines()
    assert names == sorted(names), "the written declaration must be sorted"
    assert len(names) == 4, f"one archive, its sidecar, two packages: got {names}"


def test_the_declaration_names_every_asset_the_release_stages(tmp_path: Path) -> None:
    """Declare the archives from configuration and the packages from the tree."""
    workspace = _workspace(tmp_path)

    _ = main(_argv(workspace))

    declared = workspace["output"].read_text(encoding="utf-8").splitlines()
    assert ARCHIVE in declared, "the archive is declared from configuration"
    assert SIDECAR in declared, "each archive's sidecar accompanies it"
    assert "netsuke-0.1.0-1.x86_64.rpm" in declared, (
        "a staged package is declared under its staged name"
    )


def test_a_refused_declaration_writes_no_file(tmp_path: Path) -> None:
    """Leave no partial declaration, which a later stage could verify against.

    A shorter list would make the exact-set rule pass over a release missing an
    asset, so writing nothing is the only safe outcome.
    """
    workspace = _workspace(tmp_path, stage_packages=False)

    code = main(_argv(workspace))

    assert code == EXIT_MISSING_ASSETS, (
        f"a tree without its packages cannot declare, got {code}"
    )
    assert not workspace["output"].exists(), (
        "a refused declaration must leave no file for a later stage to read"
    )
    assert not workspace["record"].exists(), (
        "a refused declaration must leave no digest record either, or the "
        "verifier would compare the download against a record nothing vouched for"
    )


def test_the_diagnosis_names_the_under_counted_format(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Send the operator to the platform that failed, not merely to the release."""
    workspace = _workspace(tmp_path)
    _ = (workspace["dist"] / "netsuke-0.1.0-1.x86_64.rpm").unlink()

    code = main(_argv(workspace))

    assert code == EXIT_MISSING_ASSETS, (
        f"an under-counted format refuses the release, got {code}"
    )
    assert ".rpm" in capsys.readouterr().err, "the diagnostic must name the format"


def test_an_unreadable_input_is_reported_separately(tmp_path: Path) -> None:
    """Distinguish a wrong path from a release that cannot be declared."""
    workspace = _workspace(tmp_path)
    workspace["manifest"] = tmp_path / "absent-Cargo.toml"

    code = main(_argv(workspace))

    assert code == EXIT_INPUT_UNUSABLE, (
        f"a missing manifest is an input fault, not a release fault, got {code}"
    )
    assert not workspace["output"].exists(), (
        "nothing was declared, so nothing is written"
    )


def test_a_configuration_without_targets_is_an_input_fault(tmp_path: Path) -> None:
    """Report the configuration's own defect as unreadable, not as a shortfall."""
    workspace = _workspace(tmp_path)
    _ = workspace["staging"].write_text("[common]\n", encoding="utf-8")

    code = main(_argv(workspace))

    assert code == EXIT_INPUT_UNUSABLE, (
        f"a configuration with no targets names no release, got {code}"
    )


@pytest.mark.parametrize(
    "missing",
    [
        "--dist-dir",
        "--staging-config",
        "--manifest",
        "--version",
        "--output",
        "--digest-record",
    ],
)
def test_the_cli_requires_every_input(missing: str) -> None:
    """Refuse to guess any of the six, since each names a different file."""
    complete = {
        "--dist-dir": "dist",
        "--staging-config": "staging.toml",
        "--manifest": "Cargo.toml",
        "--version": "0.1.0",
        "--output": "declared.txt",
        "--digest-record": "trusted.json",
    }
    argv = [
        part
        for flag, value in complete.items()
        if flag != missing
        for part in (flag, value)
    ]

    with pytest.raises(SystemExit):
        _ = build_parser().parse_args(argv)


def test_the_written_declaration_is_what_the_verifier_checks(
    tmp_path: Path,
) -> None:
    """Round-trip the declaration through the verifier's own reader.

    The CLI writes the file and the verifier consumes it, so proving the pair
    here is what keeps the two formats from drifting apart.
    """
    workspace = _workspace(tmp_path)
    _stage_archive(workspace["dist"])
    _ = main(_argv(workspace))
    declared = workspace["output"].read_text(encoding="utf-8").splitlines()
    trusted = release_digest_record_files.read_digest_record(workspace["record"])

    verified = verify(workspace["dist"], expected=declared, trusted=trusted)

    assert sorted(verified) == sorted(declared), (
        "every declared asset must verify, or the producer and the verifier "
        "disagree about the release"
    )


def test_the_producer_records_a_digest_for_every_declared_asset(
    tmp_path: Path,
) -> None:
    """Record the staged bytes of every declared name, and nothing else.

    This is the round trip the trusted comparison rests on: the record the
    producer writes is read by the verifier, so a name in one and not the other
    would make every release fail its verification.
    """
    workspace = _workspace(tmp_path)
    _stage_archive(workspace["dist"])
    _ = main(_argv(workspace))
    declared = workspace["output"].read_text(encoding="utf-8").splitlines()

    recorded = release_digest_record_files.read_digest_record(workspace["record"])

    assert sorted(recorded) == sorted(declared), (
        "the record must cover exactly the declared assets, or the verifier's "
        f"exact-set check will refuse a correct release: {sorted(recorded)}"
    )
    for name in declared:
        assert recorded[name] == release_digest_files.digest_of_file(
            workspace["dist"] / name
        ), f"the recorded digest for {name} must describe its staged bytes"


def test_the_verifier_rejects_a_tree_the_declaration_names(
    tmp_path: Path,
) -> None:
    """Fail closed when a declared sidecar is absent from the staged tree.

    This is the 2026-10-12 defect stated as an assertion: the sidecar arrives,
    the archive it describes does not, and exact-set equality is what makes
    that visible.
    """
    workspace = _workspace(tmp_path)
    _stage_archive(workspace["dist"])
    _ = main(_argv(workspace))
    declared = workspace["output"].read_text(encoding="utf-8").splitlines()
    trusted = release_digest_record_files.read_digest_record(workspace["record"])
    _ = (workspace["dist"] / SIDECAR).unlink()

    with pytest.raises(release_inventory.MissingAssetError):
        _ = verify(workspace["dist"], expected=declared, trusted=trusted)
