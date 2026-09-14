#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# dependencies = ["cyclopts==4.25.2", "cuprum @ git+https://github.com/leynos/cuprum@a2134c7a3966b224eaed917efb94f8090ce5104a"]
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

import pathlib
import sys
import typing as typ

from ci_support import (
    CiScriptError,
    append_github_path,
    catalogue,
    describe_failure,
    download,
    extract_members,
    is_executable_file,
    prepend_path,
    run,
    verify_sha256,
)

import cyclopts
from cyclopts import App, Parameter

CARGO = "cargo"
ALLOWED = catalogue(CARGO)
FRONTEND_MEMBERS = ("cargo-kani", "kani")
DRIVER = pathlib.PurePosixPath("bin/kani-driver")
DEFAULT_TARGET = "x86_64-unknown-linux-gnu"
DEFAULT_QUICKINSTALL_ROOT = "https://github.com/cargo-bins/cargo-quickinstall"
DEFAULT_UPSTREAM_ROOT = "https://github.com/model-checking/kani"

app = App(config=cyclopts.config.Env("INPUT_", command=False))


def check_pinned_version(version_file: pathlib.Path, kani_version: str) -> None:
    """Fail unless ``version_file`` holds exactly ``kani_version``."""
    pinned = version_file.read_text(encoding="utf-8").strip()
    if pinned != kani_version:
        message = (
            f"{version_file} pins Kani {pinned!r} but the workflow passed "
            f"{kani_version!r}; update both in the same commit"
        )
        raise CiScriptError(message)


def frontend_url(root: str, version: str, target: str) -> str:
    """Return the cargo-quickinstall front-end archive URL."""
    archive = f"kani-verifier-{version}-{target}.tar.gz"
    return f"{root}/releases/download/kani-verifier-{version}/{archive}"


def bundle_url(root: str, version: str, target: str) -> str:
    """Return the upstream verifier bundle URL."""
    return f"{root}/releases/download/kani-{version}/kani-{version}-{target}.tar.gz"


def fetch_verified(url: str, sha256: str, scratch: pathlib.Path) -> pathlib.Path:
    """Download ``url`` into ``scratch`` and verify it against ``sha256``."""
    archive = scratch / pathlib.PurePosixPath(url).name
    download(url, archive)
    verify_sha256(archive, sha256)
    return archive


def install_frontend(archive: pathlib.Path, frontend_bin: pathlib.Path) -> None:
    """Extract the two front-end executables into ``frontend_bin``."""
    extract_members(archive, frontend_bin, FRONTEND_MEMBERS)


def install_bundle(archive: pathlib.Path) -> None:
    """Install the verifier bundle through ``cargo kani setup``."""
    result = run(
        CARGO, "kani", "setup", "--use-local-bundle", str(archive), allowed=ALLOWED
    )
    if not result.ok:
        message = f"installing the verifier bundle failed: {describe_failure(result)}"
        raise CiScriptError(message)


class Homes(typ.NamedTuple):
    """The version-qualified directories the two payloads occupy."""

    frontend_bin: pathlib.Path
    kani_dir: pathlib.Path

    def frontend_present(self) -> bool:
        """Return whether both front-end executables are in place."""
        return all(is_executable_file(self.frontend_bin / m) for m in FRONTEND_MEMBERS)

    def bundle_present(self) -> bool:
        """Return whether the verifier driver is in place."""
        return is_executable_file(self.kani_dir / DRIVER)


# One keyword-only parameter per environment input is the Cyclopts contract.
@app.default
def main(  # pylint: disable=too-many-arguments  # one parameter per input
    *,
    kani_version: typ.Annotated[str, Parameter(required=True)],
    frontend_sha256: typ.Annotated[str, Parameter(required=True)],
    bundle_sha256: typ.Annotated[str, Parameter(required=True)],
    cargo_home: typ.Annotated[pathlib.Path, Parameter(env_var="CARGO_HOME")],
    kani_home: typ.Annotated[pathlib.Path, Parameter(env_var="KANI_HOME")],
    rustup_home: typ.Annotated[pathlib.Path, Parameter(env_var="RUSTUP_HOME")],
    runner_temp: typ.Annotated[pathlib.Path, Parameter(env_var="RUNNER_TEMP")],
    github_path: typ.Annotated[pathlib.Path, Parameter(env_var="GITHUB_PATH")],
    version_file: pathlib.Path = pathlib.Path("tools/kani/VERSION"),
    target: str = DEFAULT_TARGET,
    quickinstall_root: str = DEFAULT_QUICKINSTALL_ROOT,
    upstream_root: str = DEFAULT_UPSTREAM_ROOT,
) -> int:
    """Install whichever Kani payloads the cache did not restore.

    Parameters
    ----------
    kani_version
        The pinned Kani release; must equal ``version_file``.
    frontend_sha256
        Hex SHA-256 of the cargo-quickinstall front-end archive.
    bundle_sha256
        Hex SHA-256 of the upstream verifier bundle.
    cargo_home
        The job's ``CARGO_HOME``; the front-end lands under ``frontend/``.
    kani_home
        The job's ``KANI_HOME``; the bundle is installed beneath it.
    rustup_home
        The job's ``RUSTUP_HOME``, created so the setup has somewhere to write.
    runner_temp
        Scratch space for the downloaded archives.
    github_path
        The ``GITHUB_PATH`` file that publishes the front-end directory.
    version_file
        The repository's pinned version file.
    target
        The Rust target triple the archives are built for.
    quickinstall_root
        The cargo-quickinstall repository URL; tests use a ``file://`` root.
    upstream_root
        The Kani repository URL; tests use a ``file://`` root.

    Returns
    -------
    int
        ``0`` when both payloads are in place, ``1`` otherwise.
    """
    homes = Homes(
        cargo_home / "frontend" / f"kani-{kani_version}",
        kani_home / f"kani-{kani_version}",
    )
    try:
        check_pinned_version(version_file, kani_version)
        for directory in (homes.frontend_bin, kani_home, rustup_home):
            directory.mkdir(parents=True, exist_ok=True)
        prepend_path(homes.frontend_bin)
        append_github_path(homes.frontend_bin, github_path)
        if homes.frontend_present():
            print(f"front-end {kani_version} present in {homes.frontend_bin}")
        else:
            url = frontend_url(quickinstall_root, kani_version, target)
            install_frontend(
                fetch_verified(url, frontend_sha256, runner_temp), homes.frontend_bin
            )
            print(f"installed front-end {kani_version} from {url}")
        if homes.bundle_present():
            print(f"verifier bundle {kani_version} present in {homes.kani_dir}")
        else:
            url = bundle_url(upstream_root, kani_version, target)
            install_bundle(fetch_verified(url, bundle_sha256, runner_temp))
            print(f"installed verifier bundle {kani_version} from {url}")
    except (CiScriptError, OSError) as error:
        print(f"install_kani: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    app()
