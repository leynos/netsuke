"""Build deterministic package records and archives for publisher tests."""

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


def archive_bytes(
    *,
    name: str = PACKAGE_NAME,
    version: str = VERSION,
    manifest_version: str | None = None,
    manifests: tuple[str, ...] | None = None,
    readme: bytes | None = b"# Netsuke\n\nPackaged documentation.\n",
) -> bytes:
    """Return a gzip tar archive with configurable manifests and README.

    Passing ``readme=None`` omits the README member entirely, which lets a
    caller exercise the metadata module's fallback and absence paths.

    Returns
    -------
    bytes
        The gzip-compressed tar archive a caller can write as a ``.crate``.
    """
    root = f"{name}-{version}"
    packaged_version = manifest_version if manifest_version is not None else version
    packaged_manifests = (
        manifests
        if manifests is not None
        else (manifest_text(name=name, version=packaged_version),)
    )
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz") as archive:
        members = [
            (
                "Cargo.toml" if index == 0 else f"duplicate-{index}/Cargo.toml",
                content.encode(),
            )
            for index, content in enumerate(packaged_manifests)
        ]
        if readme is not None:
            members.append(("README.md", readme))
        for filename, content in members:
            encoded = io.BytesIO(content)
            info = tarfile.TarInfo(f"{root}/{filename}")
            info.size = len(content)
            archive.addfile(info, encoded)
    return output.getvalue()


def write_candidate(
    directory: pathlib.Path,
    *,
    identity: dict[str, object] | None = None,
    crate_name: str | None = None,
    crate_bytes: bytes | None = None,
    sha256: object = None,
) -> tuple[pathlib.Path, pathlib.Path]:
    """Write one packaged identity record and its candidate archive.

    ``sha256`` overrides the digest recorded for the archive. It defaults to
    the true archive digest; pass a ``str`` for a malformed digest or ``None``
    to omit the field, so the caller can exercise the record's digest guards.

    Returns
    -------
    tuple[pathlib.Path, pathlib.Path]
        The written identity record and the candidate archive beside it.
    """
    selected_name = crate_name or f"{PACKAGE_NAME}-{VERSION}.crate"
    actual_name = (
        selected_name
        if "/" not in selected_name and "\\" not in selected_name
        else f"{PACKAGE_NAME}-{VERSION}.crate"
    )
    selected_identity = identity or {
        "tag": TAG,
        "commit": COMMIT,
        "is_prerelease": False,
        "package_name": PACKAGE_NAME,
        "version": VERSION,
        "bin_name": BIN_NAME,
    }
    body = crate_bytes if crate_bytes is not None else archive_bytes()
    archive = directory / actual_name
    _ = archive.write_bytes(body)
    record = directory / "identity.json"
    recorded = {
        "crate": selected_name,
        "identity": selected_identity,
        "sha256": hashlib.sha256(body).hexdigest(),
    }
    if sha256 is not None:
        recorded["sha256"] = sha256
    _ = record.write_text(json.dumps(recorded), encoding="utf-8")
    return record, archive
