"""Stage the tree, record, and command line the selection CLI's suite drives.

The cases in ``test_select_release_payload_cli.py`` all read a tree the release
downloaded, run the CLI over it, and assert on what was left behind. What they
vary is which contradiction the tree carries: a stale candidate, a foreign run,
an ambiguous name, replaced bytes. The staging that surrounds each variation is
identical, so it lives here.

The one shape that is not incidental is :class:`StagedTree`. It types the paths
a case reads as ``Path`` rather than as bare ``object``, so a case that divides
a payload path out of it, or asks whether the output exists, is checked against
the value it actually gets. A ``dict[str, object]`` would leave every such use
unsupported at analysis time and silently unchecked.

Nothing here invokes the CLI's decision logic directly: each helper either
writes a file or builds the argument list, and the assertions belong to the
cases.
"""

import dataclasses
import json
import typing as typ

import release_digest_files
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


class StagedTree(typ.TypedDict):
    """Name one staged tree and the files the CLI reads and writes.

    Attributes
    ----------
    dist
        Directory holding the staged tree the producers uploaded.
    staging_config
        The release staging configuration the CLI judges the tree against.
    manifest
        The crate manifest carrying the version.
    output
        Directory the CLI writes the payload and the trusted record into.
    record
        The trusted digest record the CLI writes.
    """

    dist: pathlib.Path
    staging_config: pathlib.Path
    manifest: pathlib.Path
    output: pathlib.Path
    record: pathlib.Path


def staged_tree(tmp_path: pathlib.Path, *, producer: bool = True) -> StagedTree:
    """Build a complete, publishable staged tree and its inputs.

    Returns
    -------
    StagedTree
        The paths :func:`run_cli` needs, plus the staged bytes.
    """
    dist = tmp_path / "dist"
    for name in (ARCHIVE, SIDECAR, DEB, RPM):
        stage(dist, name, name.encode("utf-8"))
    for retained in RETAINED:
        stage(dist, retained)
    if producer:
        record_provenance(dist, assets=staged_assets_of(dist))
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


def run_cli(tree: StagedTree) -> int:
    """Invoke the CLI over ``tree``.

    Returns
    -------
    int
        The CLI's exit status.
    """
    return cli.main([
        "--dist-dir",
        str(tree["dist"]),
        "--staging-config",
        str(tree["staging_config"]),
        "--manifest",
        str(tree["manifest"]),
        "--version",
        VERSION,
        "--commit",
        CANDIDATE,
        "--run-id",
        RUN_ID,
        "--output-dir",
        str(tree["output"]),
        "--digest-record",
        str(tree["record"]),
    ])


def installed_names(output: pathlib.Path) -> list[str]:
    """Return the names the CLI copied into ``output``, sorted."""
    payload = output / cli.PAYLOAD_DIRECTORY
    return sorted(path.name for path in payload.iterdir())


def staged_assets(tree: StagedTree) -> dict[str, str]:
    """Return the published name to digest mapping the tree stages.

    Returns
    -------
    dict[str, str]
        Each declared name and the digest of its staged bytes.
    """
    return staged_assets_of(tree["dist"])


def staged_assets_of(dist: pathlib.Path) -> dict[str, str]:
    """Return the digest each declared name's staged bytes have below ``dist``.

    Returns
    -------
    dict[str, str]
        Each declared name and the digest of the file staged under it.

    Notes
    -----
    A builder that is still assembling the tree has no :class:`StagedTree` to
    hand here, and refusing to describe the assets until one exists would make
    the record the tree is built *from* the last thing written.
    """
    return {
        name: release_digest_files.digest_of_file(dist / name)
        for name in (ARCHIVE, SIDECAR, DEB, RPM)
    }


__all__ = [
    "ARCHIVE",
    "CANDIDATE",
    "DEB",
    "RETAINED",
    "RPM",
    "RUN_ID",
    "SIDECAR",
    "VERSION",
    "Provenance",
    "StagedTree",
    "installed_names",
    "record_provenance",
    "run_cli",
    "stage",
    "staged_assets",
    "staged_assets_of",
    "staged_tree",
]
