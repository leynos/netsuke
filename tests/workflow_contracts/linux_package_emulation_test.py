"""Contract tests for the emulated package installation script.

The shared `validate-linux-packages` action skips both its sandbox install and
its architecture check when the package architecture differs from the runner's.
That is the arm64 lane on an x64 release runner, so
`install-emulated-package.sh` is what installs and verifies the packages the
action would have skipped.

These tests hold the script to that job: choose the install command from the
package format, run it in the architecture-tagged image that was built for the
package, and fail loudly rather than passing vacuously. Run via
`make test-workflow-contracts`.
"""

import subprocess  # ruff: ignore[suspicious-subprocess-import] - the script under test is checked in.
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / ".github" / "package-validation" / "install-emulated-package.sh"


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
