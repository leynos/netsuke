#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts==4.25.3", "cuprum==0.1.0"]
# ///
"""Install the pinned actionlint release, reusing a cached binary that matches.

The Linux gate cache owns ``./actionlint``. A warm run finds it present and,
when it reports the pinned version, keeps it. Otherwise the pinned Linux amd64
release archive is downloaded, its SHA-256 is verified against the pin, and
the single ``actionlint`` member is extracted as a regular executable. No
upstream installer script runs: the archive is the whole artefact, and the
checksum is what makes it trusted.

Parameters arrive from the environment: ``INPUT_ACTIONLINT_VERSION`` and
``INPUT_ACTIONLINT_SHA256`` are the pins the workflow declares beside the
step; ``INPUT_INSTALL_DIR`` and ``INPUT_RELEASE_ROOT`` override the defaults.
"""

import dataclasses
import pathlib
import sys
import tempfile
import typing as typ

from ci_archive import download, extract_members, verify_sha256
from ci_support import (
    CiScriptError,
    RunSpec,
    catalogue,
    describe_failure,
    is_executable_file,
    run,
)

import cyclopts
from cyclopts import App, Parameter

BINARY_NAME = "actionlint"
DEFAULT_RELEASE_ROOT = "https://github.com/rhysd/actionlint/releases/download"
DEFAULT_PLATFORM = "linux_amd64"

app = App(config=cyclopts.config.Env("INPUT_", command=False))


@dataclasses.dataclass(frozen=True, slots=True)
class Release:
    """One actionlint release: its pins, where it comes from, and where it lands.

    The fields are the step's inputs and nothing else, so the entry point takes
    one parameter rather than one per input. Every field names its environment
    variable outright: a nested dataclass field is not a command path
    component, so the app-level ``INPUT_`` prefix does not supply the rest of
    the name, and leaving it implicit would leave these unbound.
    """

    version: typ.Annotated[
        str, Parameter(env_var="INPUT_ACTIONLINT_VERSION", required=True)
    ]
    sha256: typ.Annotated[
        str, Parameter(env_var="INPUT_ACTIONLINT_SHA256", required=True)
    ]
    install_dir: typ.Annotated[pathlib.Path, Parameter(env_var="INPUT_INSTALL_DIR")] = (
        pathlib.Path()
    )
    release_root: typ.Annotated[str, Parameter(env_var="INPUT_RELEASE_ROOT")] = (
        DEFAULT_RELEASE_ROOT
    )
    platform: typ.Annotated[str, Parameter(env_var="INPUT_PLATFORM")] = DEFAULT_PLATFORM
    runner_temp: typ.Annotated[
        pathlib.Path | None, Parameter(env_var="RUNNER_TEMP")
    ] = None

    @property
    def url(self) -> str:
        """The release archive URL for this version and platform."""
        return (
            f"{self.release_root}/v{self.version}/"
            f"{BINARY_NAME}_{self.version}_{self.platform}.tar.gz"
        )

    @property
    def binary(self) -> pathlib.Path:
        """The cached binary's path, resolved so it is never looked up on PATH."""
        return (self.install_dir / BINARY_NAME).resolve()


def reported_version(binary: pathlib.Path) -> str | None:
    """Return the version ``binary --version`` reports, or ``None`` if it fails."""
    if not is_executable_file(binary):
        return None
    spec = RunSpec(catalogue(str(binary)), echo=False)
    result = run(str(binary), "--version", spec=spec)
    if not result.ok or not result.stdout:
        return None
    return result.stdout.splitlines()[0].strip()


def install(release: Release, scratch: pathlib.Path) -> None:
    """Download, verify, and extract the pinned archive over ``release.binary``."""
    archive = scratch / pathlib.PurePosixPath(release.url).name
    download(release.url, archive)
    verify_sha256(archive, release.sha256)
    extract_members(archive, release.binary.parent, (BINARY_NAME,))


def ensure_installed_version(binary: pathlib.Path, expected: str) -> str:
    """Return the version ``binary`` reports, failing unless it is ``expected``.

    Returns
    -------
    str
        The reported version, which equals ``expected``.

    Raises
    ------
    CiScriptError
        If the freshly extracted binary does not report the pinned version.
    """
    installed = reported_version(binary)
    if installed == expected:
        return installed
    spec = RunSpec(catalogue(str(binary)), echo=False)
    result = run(str(binary), "--version", spec=spec)
    message = (
        f"installed {BINARY_NAME} reports {installed!r}, expected "
        f"{expected!r} ({describe_failure(result)})"
    )
    raise CiScriptError(message)


@app.default
def main(*, release: Release) -> int:
    """Ensure ``release.install_dir/actionlint`` is the pinned release.

    Returns
    -------
    int
        ``0`` when the pinned binary is in place, ``1`` otherwise.
    """
    try:
        if reported_version(release.binary) == release.version:
            print(f"{BINARY_NAME} {release.version} restored from the cache volume")
            return 0
        with tempfile.TemporaryDirectory(dir=release.runner_temp) as scratch:
            install(release, pathlib.Path(scratch))
        installed = ensure_installed_version(release.binary, release.version)
        print(f"installed {BINARY_NAME} {installed} from {release.url}")
    except (CiScriptError, OSError) as error:
        print(f"install_actionlint: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    app()
