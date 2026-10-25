"""Shared helpers for the ``scripts/ci`` test modules.

The CI helper scripts are Cyclopts applications whose parameters come from
``INPUT_*`` and ambient GitHub Actions environment variables, and whose
external programs run through cuprum. These helpers load a script by name,
invoke its app so the entry point's return value comes back as an ``int``
rather than a ``SystemExit``, and build the release-archive fixtures the
installers download over ``file://`` URLs.

Run via ``make test-ci-scripts``.
"""

import importlib
import io
import pathlib
import sys
import tarfile
import typing as typ

if typ.TYPE_CHECKING:
    import types

    from cyclopts import App

CI_SCRIPT_DIRECTORY = pathlib.Path(__file__).resolve().parents[1] / "ci"
EXECUTABLE_MODE = 0o755


def load_ci_script(name: str) -> types.ModuleType:
    """Import ``scripts/ci/<name>.py`` under its own module name."""
    directory = str(CI_SCRIPT_DIRECTORY)
    if directory not in sys.path:
        sys.path.insert(0, directory)
    importlib.invalidate_caches()
    return importlib.import_module(name)


def invoke(app: App) -> int:
    """Run a script's app on the current environment and return its exit code."""
    result = app([], result_action="return_value")
    assert isinstance(result, int), (
        f"the entry point must return an int, got {result!r}"
    )
    return result


def write_executable(path: pathlib.Path, body: str) -> pathlib.Path:
    """Write a POSIX shell executable at ``path`` and return it."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(f"#!/bin/sh\n{body}\n", encoding="utf-8")
    path.chmod(EXECUTABLE_MODE)
    return path


def script_reporting(version: str) -> bytes:
    """Return the bytes of an executable that prints ``version``."""
    return f"#!/bin/sh\necho '{version}'\n".encode()


def write_tarball(
    archive: pathlib.Path,
    members: dict[str, bytes],
    *,
    symlinks: dict[str, str] | None = None,
) -> pathlib.Path:
    """Write a gzip tarball holding regular ``members`` and optional symlinks."""
    archive.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive, "w:gz") as tar:
        for name, content in members.items():
            info = tarfile.TarInfo(name)
            info.size = len(content)
            info.mode = EXECUTABLE_MODE
            tar.addfile(info, io.BytesIO(content))
        for name, target in (symlinks or {}).items():
            info = tarfile.TarInfo(name)
            info.type = tarfile.SYMTYPE
            info.linkname = target
            tar.addfile(info)
    return archive


def file_url(path: pathlib.Path) -> str:
    """Return the ``file://`` URL for ``path``."""
    return path.resolve().as_uri()
