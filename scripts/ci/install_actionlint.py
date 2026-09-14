#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts==4.25.2", "cuprum @ git+https://github.com/leynos/cuprum@a2134c7a3966b224eaed917efb94f8090ce5104a"]
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

import pathlib
import sys
import tempfile
import typing as typ

from ci_support import (
    CiScriptError,
    catalogue,
    describe_failure,
    download,
    extract_members,
    is_executable_file,
    run,
    verify_sha256,
)

import cyclopts
from cyclopts import App, Parameter

BINARY_NAME = "actionlint"
DEFAULT_RELEASE_ROOT = "https://github.com/rhysd/actionlint/releases/download"
DEFAULT_PLATFORM = "linux_amd64"

app = App(config=cyclopts.config.Env("INPUT_", command=False))


def reported_version(binary: pathlib.Path) -> str | None:
    """Return the version ``binary --version`` reports, or ``None`` if it fails."""
    if not is_executable_file(binary):
        return None
    result = run(str(binary), "--version", allowed=catalogue(str(binary)), echo=False)
    if not result.ok or not result.stdout:
        return None
    return result.stdout.splitlines()[0].strip()


def archive_url(release_root: str, version: str, platform: str) -> str:
    """Return the release archive URL for ``version`` on ``platform``."""
    return f"{release_root}/v{version}/{BINARY_NAME}_{version}_{platform}.tar.gz"


def install(
    binary: pathlib.Path,
    url: str,
    sha256: str,
    scratch: pathlib.Path,
) -> None:
    """Download, verify, and extract the release archive to ``binary``."""
    archive = scratch / pathlib.PurePosixPath(url).name
    download(url, archive)
    verify_sha256(archive, sha256)
    extract_members(archive, binary.parent, (BINARY_NAME,))


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
    result = run(str(binary), "--version", allowed=catalogue(str(binary)), echo=False)
    message = (
        f"installed {BINARY_NAME} reports {installed!r}, expected "
        f"{expected!r} ({describe_failure(result)})"
    )
    raise CiScriptError(message)


# One keyword-only parameter per environment input is the Cyclopts contract.
@app.default
def main(  # pylint: disable=too-many-arguments  # one parameter per input
    *,
    actionlint_version: typ.Annotated[str, Parameter(required=True)],
    actionlint_sha256: typ.Annotated[str, Parameter(required=True)],
    install_dir: pathlib.Path = pathlib.Path(),
    release_root: str = DEFAULT_RELEASE_ROOT,
    platform: str = DEFAULT_PLATFORM,
    runner_temp: typ.Annotated[
        pathlib.Path | None, Parameter(env_var="RUNNER_TEMP")
    ] = None,
) -> int:
    """Ensure ``install_dir/actionlint`` is the pinned release.

    Parameters
    ----------
    actionlint_version
        The release to install, for example ``1.7.12``.
    actionlint_sha256
        The hex SHA-256 of the release archive for ``platform``.
    install_dir
        Where the binary lives; the gate cache restores and saves it here.
    release_root
        The releases URL prefix; tests point it at a ``file://`` fixture.
    platform
        The archive platform suffix.
    runner_temp
        Scratch space for the download; the system default when unset.

    Returns
    -------
    int
        ``0`` when the pinned binary is in place, ``1`` otherwise.
    """
    # Resolved so the cached binary is executed by path, never looked up on PATH.
    binary = (install_dir / BINARY_NAME).resolve()
    try:
        if reported_version(binary) == actionlint_version:
            print(f"{BINARY_NAME} {actionlint_version} restored from the cache volume")
            return 0
        url = archive_url(release_root, actionlint_version, platform)
        with tempfile.TemporaryDirectory(dir=runner_temp) as scratch:
            install(binary, url, actionlint_sha256, pathlib.Path(scratch))
        installed = ensure_installed_version(binary, actionlint_version)
        print(f"installed {BINARY_NAME} {installed} from {url}")
    except CiScriptError as error:
        print(f"install_actionlint: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    app()
