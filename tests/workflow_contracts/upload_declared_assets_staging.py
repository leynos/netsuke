"""Build the staged release tree the declared-asset execution cases run against.

The execution cases in ``upload_declared_assets_execution_test.py`` drive the
real ``.github/actions/upload-declared-assets`` run bodies, so they need a tree
shaped like the one the release workflow's download and hoist steps leave
behind: one directory per configured target holding that target's producer
record and its operating-system packages, the cargo-binstall archives and their
sidecars at the ``dist/`` root, and the retained files a checkout also leaves
there.

Every name is derived from the repository's own
``.github/release-staging.toml`` and ``Cargo.toml`` rather than restated, so a
target added to the release configuration changes this fixture and the cases
together instead of quietly shrinking the tree a case believes it built.

Building the tree is the whole of this module's job. Reading the action
manifest, stubbing ``uv`` and ``gh``, and executing the steps belong to
``upload_declared_assets_harness.py``.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import hashlib
import json
import shutil
import tomllib
import typing as typ

from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    from pathlib import Path

#: The candidate commit and workflow run every staged producer record claims.
COMMIT = "a" * 40
RUN_ID = "4242"

#: The staged tree the action's ``dist-dir`` input points at, relative to the
#: workspace root the steps run in.
DIST = "dist"

#: The record each producer writes beside the artefacts it stages.
RECORD_NAME = "producer-provenance.json"

#: One package per format each platform stages, named the way the release names
#: them: the format suffix alone is what the declaration counts, so a package
#: that carried a different middle segment would still be counted and still be
#: the wrong name.
#:
#: The macOS name carries the *binary* and the matrix's suffix, because its
#: stamp step interpolates ``inputs['bin-name']`` and ``inputs['artifact-suffix']``
#: rather than the crate name -- which for this repository differ, so a fixture
#: built from the crate name would stage a package the producer never reads.
#: ``suffix`` is expanded from :data:`MACOS_SUFFIX` so the name a case stages
#: and the value it resolves ``inputs['artifact-suffix']`` to cannot drift.
PACKAGE_TEMPLATES = {
    "linux": ("{package}_{version}_{arch}.deb", "{package}-{version}-1.{arch}.rpm"),
    "macos": ("{bin_name}-{version}-{suffix}.pkg",),
    "windows": ("{package}-{version}-{arch}.msi",),
}

#: The ``artifact-suffix`` the macOS matrix passes, which its stamp step
#: interpolates into the package name. One source, because a case that
#: resolved the input to something else would stage a package the producer
#: reads under a name the case never created.
MACOS_SUFFIX = "macos-{arch}"


@dataclasses.dataclass(frozen=True, slots=True)
class Target:
    """Name one configured release target.

    Attributes
    ----------
    key
        The staging configuration's target key, such as ``linux-x86_64``.
    platform
        The platform whose package formats the target stages.
    triple
        The Rust target triple every archive name interpolates.
    arch
        The architecture token the package names use.
    """

    key: str
    platform: str
    triple: str
    arch: str


@dataclasses.dataclass(frozen=True, slots=True)
class Facts:
    """Hold the release facts the staged tree and the action's inputs share.

    Attributes
    ----------
    package
        The Cargo package name every name interpolates.
    bin_name
        The binary's name, which the macOS package name carries instead of the
        package's.
    version
        The release version every name interpolates.
    staging_template
        The configured ``staging_dir_template`` a staging directory expands.
    targets
        Every configured target, sorted by key.
    """

    package: str
    bin_name: str
    version: str
    staging_template: str
    targets: tuple[Target, ...]


def release_facts() -> Facts:
    """Read the release facts from the repository's own configuration.

    The binary name comes from the staging configuration rather than from
    ``Cargo.toml``'s ``[[bin]]``, because that is the value the release
    workflow passes as ``bin-name`` and therefore the one every name carries.

    Returns
    -------
    Facts
        The package, binary name, version, and every configured target.
    """
    config = tomllib.loads(
        (REPO_ROOT / ".github" / "release-staging.toml").read_text(encoding="utf-8")
    )
    manifest = tomllib.loads((REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    targets = tuple(
        Target(
            key=key,
            platform=str(entry["platform"]),
            triple=str(entry["target"]),
            arch=str(entry["arch"]),
        )
        for key, entry in sorted(config["targets"].items())
    )
    assert targets, "the staging configuration must declare a release target"
    return Facts(
        package=str(manifest["package"]["name"]),
        bin_name=str(config["common"]["bin_name"]),
        version=str(manifest["package"]["version"]),
        staging_template=str(config["common"]["staging_dir_template"]),
        targets=targets,
    )


def staging_directory(facts: Facts, target: Target) -> str:
    """Return the directory name the shared staging action creates.

    Returns
    -------
    str
        The configured ``staging_dir_template`` expanded for ``target``.
    """
    return facts.staging_template.format(
        bin_name=facts.bin_name, platform=target.platform, arch=target.arch
    )


def archive_name(facts: Facts, target: Target) -> str:
    """Return the cargo-binstall archive name for one target.

    Returns
    -------
    str
        The archive name the release publishes for ``target``.
    """
    return f"{facts.package}-{facts.version}-{target.triple}.tar.gz"


def package_names(facts: Facts, target: Target) -> tuple[str, ...]:
    """Return the operating-system package names one target stages.

    The binary name and the Cargo package's are not interchangeable here: this
    repository names its crate ``netsuke-build`` and ships a ``netsuke``
    binary, and the macOS package name carries the binary's.

    Returns
    -------
    tuple[str, ...]
        One name per format the target's platform stages.
    """
    templates = PACKAGE_TEMPLATES[target.platform]
    return tuple(
        template.format(
            package=facts.package,
            bin_name=facts.bin_name,
            version=facts.version,
            arch=target.arch,
            suffix=MACOS_SUFFIX.format(arch=target.arch),
        )
        for template in templates
    )


def published_names(facts: Facts, target: Target) -> tuple[str, ...]:
    """Return every name one producer publishes, archive first.

    Returns
    -------
    tuple[str, ...]
        The archive, its sidecar, and the target's packages.
    """
    archive = archive_name(facts, target)
    return (archive, f"{archive}.sha256", *package_names(facts, target))


def declared_names(facts: Facts) -> tuple[str, ...]:
    """Return the flat consumer set every configured producer declares.

    Returns
    -------
    tuple[str, ...]
        Every published name, sorted.
    """
    names = [
        name for target in facts.targets for name in published_names(facts, target)
    ]
    return tuple(sorted(names))


def sidecar(archive: str, content: bytes) -> bytes:
    """Return a checksum line describing ``content``, as the build writes it.

    Returns
    -------
    bytes
        Two-space-separated digest and archive name, newline-terminated.
    """
    return f"{hashlib.sha256(content).hexdigest()}  {archive}\n".encode()


def _record(target: Target, assets: dict[str, str]) -> dict[str, object]:
    """Assemble one producer's record over the assets it stages.

    Returns
    -------
    dict[str, object]
        A schema-version-1 producer record for ``target``.
    """
    return {
        "schema": 1,
        "commit": COMMIT,
        "run_id": RUN_ID,
        "run_attempt": "1",
        "target": target.key,
        "platform": target.platform,
        "assets": dict(sorted(assets.items())),
    }


def _stage_dist(workspace: Path, facts: Facts) -> None:
    """Lay out the tree the download and hoist steps leave behind."""
    dist = workspace / DIST
    dist.mkdir()
    (dist / "LICENSE").write_bytes(b"retained licence\n")
    for target in facts.targets:
        directory = dist / target.key
        directory.mkdir()
        (directory / "netsuke").write_bytes(b"retained binary\n")
        archive = archive_name(facts, target)
        content = f"{archive}\n".encode()
        sidecar_bytes = sidecar(archive, content)
        (dist / archive).write_bytes(content)
        (dist / f"{archive}.sha256").write_bytes(sidecar_bytes)
        digests = {
            archive: hashlib.sha256(content).hexdigest(),
            f"{archive}.sha256": hashlib.sha256(sidecar_bytes).hexdigest(),
        }
        for name in package_names(facts, target):
            package = f"{name}\n".encode()
            (directory / name).write_bytes(package)
            digests[name] = hashlib.sha256(package).hexdigest()
        (directory / RECORD_NAME).write_text(
            json.dumps(_record(target, digests), indent=2) + "\n", encoding="utf-8"
        )


def stage_repository(tmp_path: Path, *, name: str = "workspace") -> Path:
    """Lay out the repository files a release step reads, and nothing else.

    The scripts are copied because the steps invoke them by their
    repository-relative path; they are read from the real checkout so the cases
    exercise the code under review rather than a stand-in. The staged tree is
    *not* created: a case that runs a producer builds that producer's own tree,
    and one that runs the action builds the downloaded union.

    Returns
    -------
    Path
        The workspace root the steps run in.
    """
    workspace = tmp_path / name
    (workspace / "scripts").mkdir(parents=True)
    for module in sorted((REPO_ROOT / "scripts").glob("*.py")):
        shutil.copy2(module, workspace / "scripts" / module.name)
    (workspace / ".github").mkdir()
    shutil.copy2(
        REPO_ROOT / ".github" / "release-staging.toml",
        workspace / ".github" / "release-staging.toml",
    )
    shutil.copy2(REPO_ROOT / "Cargo.toml", workspace / "Cargo.toml")
    return workspace


def stage_workspace(tmp_path: Path, facts: Facts) -> Path:
    """Lay out the workspace the action runs in, with a complete staged tree.

    Returns
    -------
    Path
        The workspace root the steps run in.
    """
    workspace = stage_repository(tmp_path)
    _stage_dist(workspace, facts)
    return workspace


def record_path(workspace: Path, target: Target) -> Path:
    """Return the producer record path for one target.

    Returns
    -------
    Path
        The record the target's producer wrote.
    """
    return workspace / DIST / target.key / RECORD_NAME


def rewrite_record(workspace: Path, target: Target, **changes: str) -> None:
    """Replace named members of one producer record, leaving the rest alone."""
    path = record_path(workspace, target)
    record = json.loads(path.read_text(encoding="utf-8"))
    record.update(changes)
    path.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")


def recorded_digests(workspace: Path) -> dict[str, str]:
    """Return every digest the staged producer records claim, as a union.

    Returns
    -------
    dict[str, str]
        Published asset name mapped to its recorded digest.
    """
    digests: dict[str, str] = {}
    for path in sorted((workspace / DIST).rglob(RECORD_NAME)):
        digests.update(json.loads(path.read_text(encoding="utf-8"))["assets"])
    return digests
