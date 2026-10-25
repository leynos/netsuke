#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts==4.25.3", "cuprum==0.1.0"]
# ///
"""Install the pinned actionlint release, reusing a cached binary that matches.

The Linux gate cache owns ``./actionlint``. A warm run finds it present and,
when it reports the pinned version, keeps it. A binary that is present but
cannot answer its version probe fails the step instead of being replaced, so a
broken cache is reported rather than silently repaired. Otherwise the pinned
Linux amd64 release archive is downloaded, its SHA-256 is verified against the
pin, and the single ``actionlint`` member is extracted as a regular executable.
No upstream installer script runs: the archive is the whole artefact, and the
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
        """The install path without following its final component."""
        return self.install_dir / BINARY_NAME


class ActionlintSymlinkInstallError(CiScriptError):
    """Refuse actionlint installation through a symbolic link."""

    def __init__(self, binary: pathlib.Path) -> None:
        """Build the refusal message for ``binary``."""
        super().__init__(
            f"refusing to install actionlint through symbolic link {binary}"
        )


@dataclasses.dataclass(frozen=True, slots=True)
class Probe:
    """The outcome of asking a binary for its version.

    Three states, and the caller reads them in this order:

    - ``failure`` set: the binary is there but cannot answer, so the cache is
      broken and the step must fail rather than paper over it.
    - ``version`` set: the binary answered with that version.
    - neither set: there is nothing to ask, which is an empty cache.

    The first and last are the pair worth keeping apart. An empty cache is what
    the cold path exists for; a binary that runs and cannot report a version is
    a different fact, and a caller that cannot tell them apart has to guess --
    and the guess that replaces a broken binary hides the breakage behind a
    green step.
    """

    version: str | None = None
    failure: str | None = None


def probe_version(binary: pathlib.Path) -> Probe:
    """Ask ``binary --version`` what it reports, and why if it cannot say.

    A binary that is absent, or present and not executable, is an empty cache:
    there is nothing to run, so the caller installs. A binary that runs and
    exits non-zero, or exits cleanly without printing a version, is broken,
    and the returned ``Probe`` carries the diagnostic. The two are distinct
    because a reinstall would repair neither silently: it would hide the
    second behind a green step and leave the cache volume holding an entry
    nobody knows is wrong.

    "Without printing a version" is judged on the normalized first line, not on
    the raw stream. Whitespace alone is truthy, so a stream of spaces or
    newlines passes a check on ``result.stdout`` and then strips to the empty
    string, which would be handed back as a version. No pin equals the empty
    string, so that empty version would read as a mismatch and the broken entry
    would be replaced -- the very silent repair the failure state exists to
    prevent. Split first to preserve the existing first-line parsing; stripping
    the whole stream first could promote a later line over a blank first line.

    Parameters
    ----------
    binary
        Candidate executable path. It is made absolute for command lookup, but
        its final component is never resolved through a symbolic link.

    Returns
    -------
    Probe
        The version the binary reported, or the reason it could not report
        one. Neither field is set when there was nothing to ask, which is the
        empty-cache case the cold path exists for.
    """
    binary = binary.absolute()
    if binary.is_symlink():
        return Probe(failure=f"{binary} is a symbolic link")
    if not is_executable_file(binary):
        return Probe()
    spec = RunSpec(catalogue(str(binary)), echo=False)
    result = run(str(binary), "--version", spec=spec)
    if not result.ok:
        return Probe(failure=describe_failure(result))
    lines = (result.stdout or "").splitlines()
    version = lines[0].strip() if lines else ""
    if not version:
        reason = f"{describe_failure(result)}: printed no version"
        return Probe(failure=reason)
    return Probe(version=version)


def install(release: Release, scratch: pathlib.Path) -> None:
    """Download, verify, and extract the pinned archive over ``release.binary``.

    ``INPUT_INSTALL_DIR`` may name a directory that does not exist yet, and
    extraction writes into the destination rather than creating it, so the
    directory is made here. It is made after verification rather than before
    so a checksum failure leaves no install directory behind.

    Parameters
    ----------
    release
        Pins and destinations for the actionlint archive.
    scratch
        Directory for the temporary downloaded archive.

    Raises
    ------
    ActionlintSymlinkInstallError
        If the install path is a symbolic link.
    """
    binary = release.binary
    if binary.is_symlink():
        raise ActionlintSymlinkInstallError(binary)
    archive = scratch / pathlib.PurePosixPath(release.url).name
    download(release.url, archive)
    verify_sha256(archive, release.sha256)
    binary.parent.mkdir(parents=True, exist_ok=True)
    if binary.is_symlink():
        raise ActionlintSymlinkInstallError(binary)
    extract_members(archive, binary.parent, (BINARY_NAME,))


def ensure_installed_version(binary: pathlib.Path, expected: str) -> str:
    """Return the version ``binary`` reports, failing unless it is ``expected``.

    Parameters
    ----------
    binary
        Installed executable whose version must be checked.
    expected
        Pinned version the executable must report.

    Returns
    -------
    str
        The reported version, which equals ``expected``.

    Raises
    ------
    CiScriptError
        If the freshly extracted binary cannot be asked for its version, or
        reports a different one. The failure detail comes from the same probe
        the comparison used, so the diagnostic costs no second invocation.
    """
    probe = probe_version(binary)
    if probe.version == expected:
        return expected
    detail = f" ({probe.failure})" if probe.failure else ""
    message = (
        f"installed {BINARY_NAME} reports {probe.version!r}, expected "
        f"{expected!r}{detail}"
    )
    raise CiScriptError(message)


def reuse_or_install(release: Release) -> str:
    """Return the report line for the pinned binary, installing it if needed.

    The cache probe decides the whole branch, so it lives here rather than in
    the entry point. The entry point's job is to turn a failure into an exit
    status, and the branch that reports a broken cache has to raise to do it;
    raising from inside the entry point's own ``try`` makes the decision and
    its handling one tangle, and leaves the branch untestable without going
    through the exit status.

    Parameters
    ----------
    release
        Pins, cache destination, and release source for actionlint.

    Returns
    -------
    str
        The line to print, naming either the reused cache entry or the freshly
        installed binary.

    Raises
    ------
    CiScriptError
        If the cached binary is present but cannot report a version, or if the
        freshly extracted binary reports one other than ``release.version``.
        A broken cache entry fails the step rather than being replaced, so it
        is reported where its diagnostic can name the binary; silently
        reinstalling would hide it behind a green step.
    """
    probe = probe_version(release.binary)
    if probe.failure is not None:
        message = f"the cached {BINARY_NAME} cannot be used: {probe.failure}"
        raise CiScriptError(message)
    if probe.version == release.version:
        return f"{BINARY_NAME} {release.version} restored from the cache volume"
    with tempfile.TemporaryDirectory(dir=release.runner_temp) as scratch:
        install(release, pathlib.Path(scratch))
    installed = ensure_installed_version(release.binary, release.version)
    return f"installed {BINARY_NAME} {installed} from {release.url}"


@app.default
def main(*, release: Release) -> int:
    """Ensure ``release.install_dir/actionlint`` is the pinned release.

    A cached binary that cannot be asked for its version fails the step rather
    than being replaced. Reinstalling would hide the failure behind a green
    step, and the cache volume would keep the broken entry until the version
    pin changed, so the failure would return with nothing recorded to explain
    it. An absent or non-executable binary is the ordinary empty-cache case and
    is still replaced.

    Parameters
    ----------
    release
        Workflow inputs describing the pinned actionlint release.

    Returns
    -------
    int
        ``0`` when the pinned binary is in place, ``1`` otherwise.
    """
    try:
        print(reuse_or_install(release))
    except (CiScriptError, OSError) as error:
        print(f"install_actionlint: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    app()
