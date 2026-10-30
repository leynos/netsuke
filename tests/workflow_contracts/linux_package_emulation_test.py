"""Contract tests for emulated package installation.

The shared `validate-linux-packages` action skips both its sandbox install and
its architecture check when the package architecture differs from the runner's.
That is the arm64 lane on an x64 release runner, so the workflow's emulation
step and `install-emulated-package.sh` install and verify the packages the
action would have skipped.

These tests hold that pair to the job: require both package formats before
installing anything, choose the install command from the package format, run
it in the architecture-tagged image that was built for the package, and fail
loudly rather than passing vacuously. Run via `make test-workflow-contracts`.
"""

import subprocess  # ruff: ignore[suspicious-subprocess-import] - the script under test is checked in.
import typing as typ

import pytest
from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

SCRIPT = REPO_ROOT / ".github" / "package-validation" / "install-emulated-package.sh"
EMULATION_STEP = "Install packages under emulation"


def _run_script(
    tmp_path: Path, *args: str
) -> tuple[subprocess.CompletedProcess[str], list[str]]:
    """Run the script with a Podman stub and return its call log.

    The stub records each invocation so a test can prove which image,
    architecture, and command the script chose without needing Podman or
    QEMU on the host running the suite.

    Returns
    -------
    tuple[subprocess.CompletedProcess[str], list[str]]
        The completed process and one line per recorded Podman call.
    """
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    log = tmp_path / "podman.log"
    stub = bin_dir / "podman"
    stub.write_text(
        f'#!/bin/sh\nprintf "%s\\n" "$*" >> "{log}"\n',
        encoding="utf-8",
    )
    stub.chmod(0o755)
    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the script under test is checked in.
        ["/bin/sh", str(SCRIPT), *args],
        check=False,
        capture_output=True,
        text=True,
        env={"PATH": f"{bin_dir}:/usr/bin:/bin"},
    )
    calls = log.read_text(encoding="utf-8").splitlines() if log.exists() else []
    return result, calls


@pytest.mark.parametrize(
    ("suffix", "install"),
    [
        (".deb", "dpkg -i"),
        (".rpm", "rpm -i --nodeps --nosignature"),
    ],
)
def test_installs_each_format_in_the_architecture_image(
    tmp_path: Path, suffix: str, install: str
) -> None:
    """Install the package with the format's tool, then run the verifier.

    The architecture must select the image built for the package, or an x64
    image would run under an arm64 tag and quietly test the wrong thing. The
    RPM install skips dependency resolution because its image deliberately
    withholds `ninja-build`; the verifier reads the declared requirements
    instead.
    """
    package = tmp_path / f"netsuke_0.1.0-1_arm64{suffix}"
    package.write_bytes(b"")
    image = "localhost/netsuke-package-validation:arm64"
    result, calls = _run_script(tmp_path, image, "arm64", str(package), "netsuke")
    assert result.returncode == 0, (
        f"installing {suffix} must succeed, got {result.returncode}: "
        f"{result.stdout}{result.stderr}"
    )
    assert len(calls) == 1, f"the script must run Podman once, got {calls!r}"
    call = calls[0]
    fragments = (
        "--arch arm64",
        f"--volume {tmp_path}:/packages:ro,z",
        image,
        install,
        "/usr/local/bin/verify-installed-package",
        package.name,
        "netsuke",
    )
    for fragment in fragments:
        assert fragment in call, (
            f"the Podman call must contain {fragment!r}, got {call!r}"
        )
    # The stub records the command without running it, so the fragments above
    # only prove both commands are present. The contract is install *then*
    # verify in one container: a swapped order would verify against a package
    # that is not installed.
    verifier = "/usr/local/bin/verify-installed-package"
    assert call.index(install) < call.index(verifier), (
        f"the package must be installed before the verifier runs, got {call!r}"
    )


@pytest.mark.parametrize(
    "present",
    ["netsuke_0.1.0-1_arm64.deb", "netsuke-0.1.0-1.aarch64.rpm"],
    ids=["only-deb", "only-rpm"],
)
def test_workflow_step_requires_both_package_formats(
    tmp_path: Path, present: str
) -> None:
    """Fail the lane when packaging produced only one of the two formats.

    On a cross-architecture lane this step is the only install the packages
    get, so a single-format `dist` must stop the lane rather than publish the
    other format having never run. The step exits before it invokes Podman, so
    this case needs no container tooling.
    """
    dist = tmp_path / "dist"
    dist.mkdir()
    (dist / present).write_bytes(b"")
    steps = job_steps(load_workflow(PACKAGE_WORKFLOW_PATH), "build")
    script = str(named_step(steps, EMULATION_STEP)["run"])
    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the step's own script is under test.
        ["/bin/bash", "-c", script],
        cwd=tmp_path,
        check=False,
        capture_output=True,
        text=True,
        env={
            "PATH": "/usr/bin:/bin",
            "PACKAGE_ARCH": "arm64",
            "DEB_IMAGE": "localhost/netsuke-package-validation-deb:arm64",
            "RPM_IMAGE": "localhost/netsuke-package-validation-rpm:arm64",
            "BIN_NAME": "netsuke",
        },
    )
    assert result.returncode == 1, (
        f"a dist holding only {present!r} must fail the step, got "
        f"{result.returncode}: {result.stdout}{result.stderr}"
    )
    assert "::error title=Missing package format::" in result.stdout, (
        f"the step must report the missing format, got {result.stdout!r}"
    )


def test_fails_when_the_package_is_absent(tmp_path: Path) -> None:
    """Fail rather than reporting a clean run for a package that is not there."""
    result, calls = _run_script(
        tmp_path,
        "localhost/image:arm64",
        "arm64",
        str(tmp_path / "absent.deb"),
        "netsuke",
    )
    assert result.returncode == 1, (
        f"a missing package must fail the script, got {result.returncode}"
    )
    assert "FAIL: no package at" in result.stderr, (
        f"the failure must name the missing package, got {result.stderr!r}"
    )
    assert calls == [], "a missing package must not reach Podman"


def test_fails_on_an_unsupported_package_format(tmp_path: Path) -> None:
    """Fail clearly rather than guessing an install command for another format."""
    package = tmp_path / "netsuke.tar.gz"
    package.write_bytes(b"")
    result, calls = _run_script(
        tmp_path, "localhost/image:arm64", "arm64", str(package), "netsuke"
    )
    assert result.returncode == 1, (
        f"an unsupported format must fail the script, got {result.returncode}"
    )
    assert "is neither a .deb nor a .rpm" in result.stderr, (
        f"the failure must name the unsupported format, got {result.stderr!r}"
    )
    assert calls == [], "an unsupported format must not reach Podman"


def test_rejects_missing_arguments(tmp_path: Path) -> None:
    """Require every argument instead of installing a package it cannot name."""
    result, calls = _run_script(tmp_path, "localhost/image:arm64")
    assert result.returncode == 64, (
        f"missing arguments must exit 64, got {result.returncode}: {result.stderr}"
    )
    assert "Usage:" in result.stderr, (
        f"the script must print its usage, got {result.stderr!r}"
    )
    assert calls == [], "missing arguments must not reach Podman"
