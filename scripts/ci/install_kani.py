#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts==4.25.3", "cuprum==0.1.0"]
# ///
"""Install the pinned Kani front-end and verifier bundle from release archives.

Kani is two payloads. The ``cargo-kani`` and ``kani`` front-end binaries come
from the cargo-quickinstall release archive; the verifier bundle comes from
the upstream Kani release and is installed with ``cargo kani setup
--use-local-bundle``. Both archives are pinned by SHA-256 and land under
version-qualified directories, so a version bump can never reuse a stale
payload, and each is downloaded only when its executables are absent, so a
warm cache restore skips the network entirely.

Parameters arrive from the environment: ``INPUT_KANI_VERSION``,
``INPUT_FRONTEND_SHA256``, and ``INPUT_BUNDLE_SHA256`` are the pins the
workflow declares beside the step; ``CARGO_HOME``, ``KANI_HOME``,
``RUSTUP_HOME``, ``RUNNER_TEMP``, and ``GITHUB_PATH`` are the job's ambient
variables. The version must equal ``tools/kani/VERSION`` so the cache key,
the Makefile, and this installer cannot drift.
"""

import dataclasses
import pathlib
import sys
import typing as typ

from ci_archive import download, extract_members, verify_sha256
from ci_support import (
    CiScriptError,
    RunSpec,
    append_github_path,
    catalogue,
    describe_failure,
    is_executable_file,
    prepend_path,
    run,
)

import cyclopts
from cyclopts import App, Parameter

if typ.TYPE_CHECKING:
    import collections.abc as cabc

CARGO = "cargo"
ALLOWED = catalogue(CARGO)
FRONTEND_MEMBERS = ("cargo-kani", "kani")
DRIVER = pathlib.PurePosixPath("bin/kani-driver")
DEFAULT_TARGET = "x86_64-unknown-linux-gnu"
DEFAULT_QUICKINSTALL_ROOT = "https://github.com/cargo-bins/cargo-quickinstall"
DEFAULT_UPSTREAM_ROOT = "https://github.com/model-checking/kani"

app = App(config=cyclopts.config.Env("INPUT_", command=False))


@dataclasses.dataclass(frozen=True, slots=True)
class Payload:
    """One Kani archive: its release prefix, its repository, and its checksum.

    The digest is enforced before the archive is unpacked, and it travels with
    the payload rather than beside it so a mismatch always names the archive it
    belongs to instead of reporting a bare digest comparison.
    """

    prefix: str
    root: str
    sha256: str

    def path(self, version: str, target: str) -> str:
        """Return the archive's file name for ``version`` on ``target``."""
        return f"{self.prefix}-{version}-{target}.tar.gz"

    def url(self, version: str, target: str) -> str:
        """Return the archive's release URL for ``version`` on ``target``."""
        return (
            f"{self.root}/releases/download/{self.prefix}-{version}/"
            f"{self.path(version, target)}"
        )


@dataclasses.dataclass(frozen=True, slots=True)
class Homes:
    """The version-qualified directories the two payloads occupy."""

    frontend_bin: pathlib.Path
    kani_dir: pathlib.Path

    def frontend_present(self) -> bool:
        """Return whether both front-end executables are in place."""
        return all(is_executable_file(self.frontend_bin / m) for m in FRONTEND_MEMBERS)

    def bundle_present(self) -> bool:
        """Return whether the verifier driver is in place."""
        return is_executable_file(self.kani_dir / DRIVER)


@dataclasses.dataclass(frozen=True, slots=True)
class JobEnvironment:
    """The ambient job variables the installer reads and writes."""

    cargo_home: typ.Annotated[pathlib.Path, Parameter(env_var="CARGO_HOME")]
    kani_home: typ.Annotated[pathlib.Path, Parameter(env_var="KANI_HOME")]
    rustup_home: typ.Annotated[pathlib.Path, Parameter(env_var="RUSTUP_HOME")]
    runner_temp: typ.Annotated[pathlib.Path, Parameter(env_var="RUNNER_TEMP")]
    github_path: typ.Annotated[pathlib.Path, Parameter(env_var="GITHUB_PATH")]

    def homes(self, version: str) -> Homes:
        """Return the version-qualified directories this job installs into."""
        return Homes(
            self.cargo_home / "frontend" / f"kani-{version}",
            self.kani_home / f"kani-{version}",
        )

    def prepare(self, homes: Homes) -> None:
        """Create the install directories and publish the front-end to later steps."""
        for directory in (homes.frontend_bin, self.kani_home, self.rustup_home):
            directory.mkdir(parents=True, exist_ok=True)
        prepend_path(homes.frontend_bin)
        append_github_path(homes.frontend_bin, self.github_path)


@dataclasses.dataclass(frozen=True, slots=True)
class Release:
    """The pinned Kani release: its version, its checksums, and where it lands.

    The fields are the step's inputs and nothing else, so the entry point takes
    one parameter rather than one per input. Each field read from the
    environment names its variable outright: a nested dataclass field is not a
    command path component, so the app-level ``INPUT_`` prefix does not supply
    the rest of the name, and leaving it implicit would leave these unbound.
    ``target`` is not an input the workflow varies, so it stays a plain field.
    """

    version: typ.Annotated[str, Parameter(env_var="INPUT_KANI_VERSION", required=True)]
    frontend_sha256: typ.Annotated[
        str, Parameter(env_var="INPUT_FRONTEND_SHA256", required=True)
    ]
    bundle_sha256: typ.Annotated[
        str, Parameter(env_var="INPUT_BUNDLE_SHA256", required=True)
    ]
    version_file: typ.Annotated[
        pathlib.Path, Parameter(env_var="INPUT_VERSION_FILE")
    ] = pathlib.Path("tools/kani/VERSION")
    target: str = DEFAULT_TARGET
    quickinstall_root: typ.Annotated[
        str, Parameter(env_var="INPUT_QUICKINSTALL_ROOT")
    ] = DEFAULT_QUICKINSTALL_ROOT
    upstream_root: typ.Annotated[str, Parameter(env_var="INPUT_UPSTREAM_ROOT")] = (
        DEFAULT_UPSTREAM_ROOT
    )

    def frontend(self) -> Payload:
        """Return the cargo-quickinstall front-end payload for this release."""
        return Payload("kani-verifier", self.quickinstall_root, self.frontend_sha256)

    def bundle(self) -> Payload:
        """Return the upstream verifier bundle payload for this release."""
        return Payload("kani", self.upstream_root, self.bundle_sha256)


def check_pinned_version(version_file: pathlib.Path, kani_version: str) -> None:
    """Fail unless ``version_file`` holds exactly ``kani_version``."""
    pinned = version_file.read_text(encoding="utf-8").strip()
    if pinned != kani_version:
        message = (
            f"{version_file} pins Kani {pinned!r} but the workflow passed "
            f"{kani_version!r}; update both in the same commit"
        )
        raise CiScriptError(message)


def fetch_verified(
    payload: Payload, version: str, target: str, scratch: pathlib.Path
) -> pathlib.Path:
    """Download ``payload`` into ``scratch`` and verify it against its pin."""
    archive = scratch / payload.path(version, target)
    download(payload.url(version, target), archive)
    verify_sha256(archive, payload.sha256)
    return archive


def install_frontend(archive: pathlib.Path, frontend_bin: pathlib.Path) -> None:
    """Extract the two front-end executables into ``frontend_bin``."""
    extract_members(archive, frontend_bin, FRONTEND_MEMBERS)


def install_bundle(archive: pathlib.Path) -> None:
    """Install the verifier bundle through ``cargo kani setup``."""
    result = run(
        CARGO,
        "kani",
        "setup",
        "--use-local-bundle",
        str(archive),
        spec=RunSpec(ALLOWED),
    )
    if not result.ok:
        message = f"installing the verifier bundle failed: {describe_failure(result)}"
        raise CiScriptError(message)


@dataclasses.dataclass(frozen=True, slots=True)
class InstallStep:
    """One of Kani's two payloads, with the two things that differ between them.

    The payloads are otherwise identical work: skip if the cache restored the
    executables, otherwise fetch, verify, install, and report. Only what is
    checked for, what is installed, and where the executables land differ, so
    those are the fields and the rest is shared.
    """

    label: str
    destination: pathlib.Path
    is_present: cabc.Callable[[], bool]
    payload: Payload
    install: cabc.Callable[[pathlib.Path], None]


def install_step(step: InstallStep, release: Release, job: JobEnvironment) -> None:
    """Install ``step``'s payload unless its executables are already present."""
    if step.is_present():
        print(f"{step.label} {release.version} present in {step.destination}")
        return
    url = step.payload.url(release.version, release.target)
    archive = fetch_verified(
        step.payload, release.version, release.target, job.runner_temp
    )
    step.install(archive)
    print(f"installed {step.label} {release.version} from {url}")


def steps(release: Release, homes: Homes) -> tuple[InstallStep, ...]:
    """Return the two payload installs, front-end first."""
    frontend_bin = homes.frontend_bin
    return (
        InstallStep(
            "front-end",
            frontend_bin,
            homes.frontend_present,
            release.frontend(),
            lambda archive: install_frontend(archive, frontend_bin),
        ),
        InstallStep(
            "verifier bundle",
            homes.kani_dir,
            homes.bundle_present,
            release.bundle(),
            install_bundle,
        ),
    )


@app.default
def main(*, release: Release, job: JobEnvironment) -> int:
    """Install whichever Kani payloads the cache did not restore.

    Returns
    -------
    int
        ``0`` when both payloads are in place, ``1`` otherwise.
    """
    try:
        check_pinned_version(release.version_file, release.version)
        homes = job.homes(release.version)
        job.prepare(homes)
        for step in steps(release, homes):
            install_step(step, release, job)
    except (CiScriptError, OSError) as error:
        print(f"install_kani: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    app()
