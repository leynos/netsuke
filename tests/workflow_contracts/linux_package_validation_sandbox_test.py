"""Contract tests for the Linux package validation sandbox.

The gate installs each package in a sandbox and runs the repository's verifier:

- a tag-pinned base image can change under the gate;
- an RPM image shipping the dependency would weaken the declared-dependency check;
- kept documentation exclusions make ``man -w`` fail for every package; and
- emulation is installed only for a package architecture that needs it.

Verifier contracts live in ``linux_package_verifier_test.py``; workflow wiring
lives in ``linux_package_validation_gate_test.py``. These cover the sandbox
itself. Run via ``make test-workflow-contracts``.
"""

import dataclasses
import re
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the step's own script is under test.
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

VALIDATION_DIR = REPO_ROOT / ".github" / "package-validation"
VERIFY_SCRIPT = VALIDATION_DIR / "verify-installed-package.sh"
POLYTHENE_WRAPPER = VALIDATION_DIR / "polythene_cli.py"
POLYTHENE_PIN = re.compile(
    r"polythene-isolate @ git\+https://github\.com/leynos/polythene\.git@([0-9a-f]{40})\b"
)
DIGEST_PIN = re.compile(r"@sha256:[0-9a-f]{64}$")
SANDBOX_STEP = "Prepare Linux package validation sandbox"
#: Every line the wrapper must keep: the entry point and metadata ``uv run``
#: needs, plus the exit-status propagation that keeps a failed validation from
#: passing the gate as a no-op.
WRAPPER_REQUIRED = (
    ("from polythene.__main__ import main", "run the Polythene CLI entry point"),
    ("requires-python", "carry PEP 723 metadata so uv provides its dependency"),
    ("raise SystemExit(main())", "propagate the CLI's exit status"),
)


@pytest.mark.parametrize(
    ("containerfile", "required", "forbidden", "exclusion_marker"),
    [
        (
            "deb.Containerfile",
            ("man-db", "ninja-build"),
            (),
            "path-exclude",
        ),
        ("rpm.Containerfile", ("man-db",), ("ninja-build",), "%_excludedocs"),
    ],
)
def test_validation_images_are_pinned_and_carry_the_right_tools(
    containerfile: str,
    required: tuple[str, ...],
    forbidden: tuple[str, ...],
    exclusion_marker: str,
) -> None:
    """Build the sandbox from a digest-pinned base image with the right tools.

    The base image must be pinned by digest so the gate cannot change under a
    tag move, and must carry the manual-page tooling: a fresh image otherwise
    excludes documentation and ``man -w`` would always fail. The dependency
    policy is per distribution: Debian installs ``ninja-build`` because
    ``dpkg -i`` refuses unmet dependencies, the RPM image withholds it, and
    the verifier reads declared metadata either way.
    """
    raw_text = (VALIDATION_DIR / containerfile).read_text(encoding="utf-8")
    # Only instructions count: a comment naming a package must neither satisfy
    # the required check nor trip the forbidden one.
    instructions = [
        line for line in raw_text.splitlines() if not line.lstrip().startswith("#")
    ]
    text = "\n".join(instructions)
    from_lines = [line for line in instructions if line.startswith("FROM ")]
    assert len(from_lines) == 1, (
        f"{containerfile} must build one stage from one base image, got {from_lines!r}"
    )
    assert DIGEST_PIN.search(from_lines[0]), (
        f"{containerfile}'s base image must be pinned by digest, got {from_lines[0]!r}"
    )
    for package in required:
        assert package in text, f"{containerfile} must install {package!r}"
    for package in forbidden:
        assert package not in text, (
            f"{containerfile} must not install {package!r}: the dependency "
            "check has to prove the package declares it"
        )
    assert exclusion_marker in text, (
        f"{containerfile} must remove the {exclusion_marker!r} documentation "
        "exclusion so the installed manual page is really present"
    )
    assert "man -w man" in text, (
        f"{containerfile} must prove manual pages resolve at build time"
    )
    for marker in (
        "COPY verify-installed-package.sh /usr/local/bin/verify-installed-package",
        "chmod 0755 /usr/local/bin/verify-installed-package",
    ):
        assert marker in text, (
            f"{containerfile} must install the verification script with {marker!r}"
        )


def _write_stub(bin_dir: Path, name: str, body: str) -> None:
    """Write an executable shell stub named ``name`` into ``bin_dir``."""
    path = bin_dir / name
    path.write_text(f"#!/bin/sh\n{body}\n", encoding="utf-8")
    path.chmod(0o755)


def test_polythene_wrapper_pins_the_local_image_fix() -> None:
    """Pin the Polythene revision that reuses locally built images.

    The shared action's own revision predates the local-image probe and would
    treat the ``localhost/`` reference as a registry host, so the wrapper pins
    a full commit SHA in PEP 723 metadata that ``uv run`` resolves at launch.
    Its filename must not be ``polythene.py``: Python puts the script's
    directory first on ``sys.path``, so that name would shadow the package.
    """
    text = POLYTHENE_WRAPPER.read_text(encoding="utf-8")
    match = POLYTHENE_PIN.search(text)
    assert match is not None, (
        "the Polythene wrapper must pin polythene-isolate to a full commit SHA "
        "of https://github.com/leynos/polythene.git"
    )
    for needle, purpose in WRAPPER_REQUIRED:
        assert needle in text, f"the wrapper must {purpose}"
    assert POLYTHENE_WRAPPER.name != "polythene.py", (
        "the wrapper must not be named polythene.py, or it shadows the package "
        "it imports"
    )


SANDBOX_STUBS = {
    "uname": "printf '%s\\n' \"$STUB_UNAME\"",
    "sudo": 'printf \'sudo %s\\n\' "$*" >> "$STUB_LOG"',
}
QEMU_INSTALL = "sudo apt-get install --no-install-recommends --yes qemu-user-static"


@dataclasses.dataclass(frozen=True, slots=True)
class SandboxCase:
    """One sandbox-preparation scenario and the command calls it permits."""

    target: str
    runner_arch: str
    podman_present: bool
    expected_status: int
    expected_arch: str | None
    expected_calls: tuple[str, ...]


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(
            SandboxCase(
                target="x86_64-unknown-linux-gnu",
                runner_arch="x86_64",
                podman_present=True,
                expected_status=0,
                expected_arch="amd64",
                expected_calls=(),
            ),
            id="native-amd64",
        ),
        pytest.param(
            SandboxCase(
                target="aarch64-unknown-linux-gnu",
                runner_arch="x86_64",
                podman_present=True,
                expected_status=0,
                expected_arch="arm64",
                expected_calls=(
                    QEMU_INSTALL,
                    "sudo systemctl restart systemd-binfmt",
                ),
            ),
            id="arm64-under-emulation",
        ),
        pytest.param(
            SandboxCase(
                target="aarch64-unknown-linux-gnu",
                runner_arch="aarch64",
                podman_present=False,
                expected_status=0,
                expected_arch="arm64",
                expected_calls=(
                    "sudo apt-get install --no-install-recommends --yes podman",
                ),
            ),
            id="native-arm64-without-podman",
        ),
        pytest.param(
            SandboxCase(
                target="riscv64gc-unknown-linux-gnu",
                runner_arch="x86_64",
                podman_present=True,
                expected_status=1,
                expected_arch=None,
                expected_calls=(),
            ),
            id="unsupported-target",
        ),
    ],
)
def test_sandbox_step_resolves_the_target_architecture(
    tmp_path: Path, case: SandboxCase
) -> None:
    """Resolve the package architecture and install only what it needs.

    Emulation is installed only when the package architecture differs from the
    runner's, so the common lane pays no QEMU cost; an unsupported target fails
    rather than silently validate an image for the wrong architecture.
    """
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    _write_stub(bin_dir, "uname", SANDBOX_STUBS["uname"])
    _write_stub(bin_dir, "sudo", SANDBOX_STUBS["sudo"])
    if case.podman_present:
        _write_stub(bin_dir, "podman", "exit 0")
    log = tmp_path / "calls.log"
    output = tmp_path / "github-output"
    output.write_text("", encoding="utf-8")
    script = str(named_step(_steps(), SANDBOX_STEP)["run"])
    result = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the step's own script is under test.
        ["/bin/bash", "-c", script],
        check=False,
        capture_output=True,
        text=True,
        env={
            "PATH": str(bin_dir),
            "TARGET": case.target,
            "GITHUB_OUTPUT": str(output),
            "STUB_UNAME": case.runner_arch,
            "STUB_LOG": str(log),
        },
    )
    assert result.returncode == case.expected_status, (
        f"{case.target!r} on a {case.runner_arch} runner must exit "
        f"{case.expected_status}, got {result.returncode}: "
        f"{result.stdout}{result.stderr}"
    )
    if case.expected_status != 0:
        assert "Unsupported Linux package target" in result.stdout + result.stderr, (
            f"an unsupported target must be named, got {result.stdout!r} and "
            f"{result.stderr!r}"
        )
        return
    runner_arch = "amd64" if case.runner_arch == "x86_64" else "arm64"
    cross_arch = "true" if case.expected_arch != runner_arch else "false"
    assert output.read_text(encoding="utf-8").splitlines() == [
        f"arch={case.expected_arch}",
        f"runner-arch={runner_arch}",
        f"cross-arch={cross_arch}",
    ], "the step must expose both architectures and whether they differ"
    calls = log.read_text(encoding="utf-8").splitlines() if log.exists() else []
    for call in case.expected_calls:
        assert call in calls, (
            f"expected the sandbox step to run {call!r}, got {calls!r}"
        )
    if not case.expected_calls:
        assert calls == [], f"the native lane must install nothing, got {calls!r}"


def _steps() -> list[dict[str, object]]:
    """Return the Linux packaging job's steps in declaration order."""
    return job_steps(load_workflow(PACKAGE_WORKFLOW_PATH), "build")
