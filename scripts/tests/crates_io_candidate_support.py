"""Build deterministic package records and archives for publisher tests."""

import dataclasses
import hashlib
import io
import json
import tarfile
import typing as typ

if typ.TYPE_CHECKING:
    import pathlib

COMMIT = "a" * 40
TAG = "v1.2.3"
PACKAGE_NAME = "netsuke-build"
VERSION = "1.2.3"
BIN_NAME = "netsuke"


def manifest_text(*, name: str = PACKAGE_NAME, version: str = VERSION) -> str:
    """Return a packaged manifest with each publish metadata category."""
    return f"""[package]
name = "{name}"
version = "{version}"
authors = ["Netsuke contributors"]
description = "A build tool"
documentation = "https://docs.example.test"
homepage = "https://example.test"
keywords = ["build", "ninja"]
categories = ["development-tools"]
license = "MIT OR Apache-2.0"
readme = "README.md"
repository = "https://github.com/leynos/netsuke"
rust-version = "1.85"
links = "netsuke_native"

[package.metadata]
[features]
default = ["fast"]
fast = []

[dependencies]
serde = {{ version = "^1.0", features = ["derive"], default-features = false }}
renamed-serde = {{ package = "serde", version = "^1.0", optional = true }}

[dev-dependencies]
rstest = "^0.26"

[build-dependencies]
cc = "^1.0"

[target.'cfg(windows)'.dependencies]
windows-sys = {{ version = "^0.59", features = ["Win32_Foundation"] }}
"""


def tar_bytes(members: tuple[tuple[str, bytes | None], ...]) -> bytes:
    """Return a gzip tar archive holding the named members in order.

    A ``None`` content records a directory entry rather than a regular file, so
    a caller can exercise the archive reader's non-regular-member guard.

    Returns
    -------
    bytes
        The gzip-compressed tar archive a caller can write as a ``.crate``.
    """
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz") as archive:
        for name, content in members:
            info = tarfile.TarInfo(name)
            if content is None:
                info.type = tarfile.DIRTYPE
                info.size = 0
                archive.addfile(info)
                continue
            info.size = len(content)
            archive.addfile(info, io.BytesIO(content))
    return output.getvalue()


def archive_bytes(
    *,
    manifest_version: str | None = None,
    manifests: tuple[str, ...] | None = None,
    readme: bytes | None = b"# Netsuke\n\nPackaged documentation.\n",
    root: str | None = None,
) -> bytes:
    """Return a gzip tar archive of the package with the chosen contents.

    ``manifest_version`` rewrites the version the manifest states while leaving
    the archive's own name alone, so a caller can make the two disagree.
    Passing ``readme=None`` omits the README member entirely, which lets a
    caller exercise the metadata module's fallback and absence paths. ``root``
    overrides the single top-level directory, so a caller can place the
    manifest outside the ``<package>-<version>`` root the candidate check
    requires.

    Returns
    -------
    bytes
        The gzip-compressed tar archive a caller can write as a ``.crate``.
    """
    archive_root = root if root is not None else f"{PACKAGE_NAME}-{VERSION}"
    packaged_version = manifest_version if manifest_version is not None else VERSION
    packaged_manifests = (
        manifests
        if manifests is not None
        else (manifest_text(version=packaged_version),)
    )
    members = [
        (
            "Cargo.toml" if index == 0 else f"duplicate-{index}/Cargo.toml",
            content.encode(),
        )
        for index, content in enumerate(packaged_manifests)
    ]
    if readme is not None:
        members.append(("README.md", readme))
    return tar_bytes(
        tuple((f"{archive_root}/{filename}", content) for filename, content in members)
    )


@dataclasses.dataclass(frozen=True, slots=True)
class CandidateOverrides:
    """Carry the record and archive substitutions a guard test needs.

    Each field defaults to the accepted candidate's value, so a caller names
    only the one detail under test. ``sha256`` defaults to the archive's true
    digest; a ``str`` records that literal value instead, which lets a caller
    express a malformed digest without naming the archive's real one.
    """

    identity: dict[str, object] | None = None
    crate_name: str | None = None
    crate_bytes: bytes | None = None
    sha256: object = None


def write_candidate(
    directory: pathlib.Path, *, overrides: CandidateOverrides | None = None
) -> tuple[pathlib.Path, pathlib.Path]:
    """Write one packaged identity record and its candidate archive.

    Returns
    -------
    tuple[pathlib.Path, pathlib.Path]
        The written identity record and the candidate archive beside it.
    """
    chosen = overrides or CandidateOverrides()
    selected_name = chosen.crate_name or f"{PACKAGE_NAME}-{VERSION}.crate"
    actual_name = (
        selected_name
        if "/" not in selected_name and "\\" not in selected_name
        else f"{PACKAGE_NAME}-{VERSION}.crate"
    )
    selected_identity = chosen.identity or {
        "tag": TAG,
        "commit": COMMIT,
        "is_prerelease": False,
        "package_name": PACKAGE_NAME,
        "version": VERSION,
        "bin_name": BIN_NAME,
    }
    body = chosen.crate_bytes if chosen.crate_bytes is not None else archive_bytes()
    archive = directory / actual_name
    _ = archive.write_bytes(body)
    record = directory / "identity.json"
    recorded: dict[str, object] = {
        "crate": selected_name,
        "identity": selected_identity,
        "sha256": hashlib.sha256(body).hexdigest(),
    }
    if chosen.sha256 is not None:
        recorded["sha256"] = chosen.sha256
    _ = record.write_text(json.dumps(recorded), encoding="utf-8")
    return record, archive
