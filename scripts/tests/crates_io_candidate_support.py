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
) -> bytes:
    """Return a gzip tar archive containing the manifest and README."""
    root = f"{name}-{version}"
    packaged_version = manifest_version if manifest_version is not None else version
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz") as archive:
        for filename, content in (
            (
                "Cargo.toml",
                manifest_text(name=name, version=packaged_version).encode(),
            ),
            ("README.md", b"# Netsuke\n\nPackaged documentation.\n"),
        ):
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
) -> tuple[pathlib.Path, pathlib.Path]:
    """Write one packaged identity record and its candidate archive."""
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
    _ = record.write_text(
        json.dumps({
            "crate": selected_name,
            "identity": selected_identity,
            "sha256": hashlib.sha256(body).hexdigest(),
        }),
        encoding="utf-8",
    )
    return record, archive
