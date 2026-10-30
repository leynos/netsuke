"""Contract tests for the Linux package validation gate.

The Linux packaging lane builds ``.deb`` and ``.rpm`` packages and, before this
gate existed, published them having only inspected their metadata. A package
can carry correct metadata and still be broken: a dependency can be missing, a
manual page can be left out of the archive, or the binary can fail to run under
the target architecture.

The gate closes that gap by installing each package in a sandbox built from a
locally built image and running the repository's verification script. These
tests hold the parts that make it a gate rather than a suggestion: the action
is pinned to the revision the packaging action uses, the validation runs
between packaging and upload, the sandbox image carries what the checks need
and withholds what they must prove, the version handed to the validator is the
one nfpm writes, and the whole lane runs in the release dry run.

The sandbox contents and the verification script are covered by
``linux_package_validation_sandbox_test.py``. The split keeps each module
inside the repository's 400-line limit.

Run via ``make test-workflow-contracts``.
"""

import subprocess  # ruff: ignore[suspicious-subprocess-import] - the step's own scripts are under test.
import typing as typ

import pytest
from action_references import require_external_action_sha
from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
    require_list,
    require_mapping,
    unique_step_index,
    workflow_job,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

PACKAGING_ACTION = "leynos/shared-actions/.github/actions/linux-packages"
VALIDATION_ACTION = "leynos/shared-actions/.github/actions/validate-linux-packages"
PACKAGING_STEP = "Package Linux artefacts with dependencies"
SANDBOX_STEP = "Prepare Linux package validation sandbox"
DEB_IMAGE_STEP = "Build Debian validation image"
RPM_IMAGE_STEP = "Build RPM validation image"
VERSION_STEP = "Normalise Linux package validation version"
VALIDATION_STEP = "Validate Linux packages"
EMULATION_STEP = "Install packages under emulation"
UPLOAD_STEP = "Upload Linux artefacts"

#: The gate's steps in the order that makes it a gate: the packages exist, the
#: sandbox exists, the validator installs them, and only then may they upload.
GATE_ORDER = (
    PACKAGING_STEP,
    SANDBOX_STEP,
    DEB_IMAGE_STEP,
    RPM_IMAGE_STEP,
    VERSION_STEP,
    VALIDATION_STEP,
    EMULATION_STEP,
    UPLOAD_STEP,
)

LINUX_ONLY = "inputs.platform == 'linux'"
CROSS_ARCH_ONLY = (
    f"{LINUX_ONLY} && steps.validation_sandbox.outputs.cross-arch == 'true'"
)
#: Every step this gate adds, with the condition that keeps it unskippable.
GATE_CONDITIONS = {
    SANDBOX_STEP: LINUX_ONLY,
    DEB_IMAGE_STEP: LINUX_ONLY,
    RPM_IMAGE_STEP: LINUX_ONLY,
    VERSION_STEP: LINUX_ONLY,
    VALIDATION_STEP: LINUX_ONLY,
    EMULATION_STEP: CROSS_ARCH_ONLY,
}
#: The steps this issue adds, each of which must be Linux-only and unskippable.
NEW_GATE_STEPS = GATE_ORDER[1:-1]

MAN_PAGE_PATH = "/usr/share/man/man1/netsuke.1.gz"


def _linux_steps() -> list[dict[str, object]]:
    """Return the Linux packaging job's steps in declaration order."""
    return job_steps(load_workflow(PACKAGE_WORKFLOW_PATH), "build")


def _inputs(step_name: str) -> dict[str, object]:
    """Return the ``with`` block of the uniquely named step."""
    step = named_step(_linux_steps(), step_name)
    return require_mapping(step.get("with"), f"the {step_name} inputs")


def test_validation_runs_between_packaging_and_upload() -> None:
    """The packages must be installed before the lane can upload them.

    Ordering is the entire point: a validator placed after the upload could
    only report a broken package that had already been published. The steps
    are also required to be Linux-only and free of ``continue-on-error``, so a
    failure stops the lane rather than annotating it.
    """
    steps = _linux_steps()
    indexes = [unique_step_index(steps, name) for name in GATE_ORDER]
    assert indexes == sorted(indexes), (
        "the package validation gate must run after packaging and before the "
        f"upload, got step order {GATE_ORDER!r} at indexes {indexes!r}"
    )
    for name in NEW_GATE_STEPS:
        step = named_step(steps, name)
        assert step.get("if") == GATE_CONDITIONS[name], (
            f"{name!r} must run unconditionally within its architecture and "
            f"must not be conditioned on the dry run or the upload toggle, got "
            f"{step.get('if')!r}"
        )
        assert "continue-on-error" not in step, (
            f"{name!r} must fail the lane rather than annotate it"
        )


def test_validation_uses_the_release_packaging_revision() -> None:
    """Validate with the same shared-actions revision that builds the packages.

    Reading both pins rather than spelling one out lets Dependabot bump them;
    what must hold is that the package under test was built by the same code
    revision the validator came from.
    """
    steps = _linux_steps()
    packaging_sha = require_external_action_sha(
        named_step(steps, PACKAGING_STEP).get("uses"),
        PACKAGING_ACTION,
        "the release lane's Linux packaging",
    )
    validation_sha = require_external_action_sha(
        named_step(steps, VALIDATION_STEP).get("uses"),
        VALIDATION_ACTION,
        "the Linux package validation gate",
    )
    assert validation_sha == packaging_sha, (
        "package validation must use the same shared-actions revision as "
        f"packaging, got {validation_sha!r} against {packaging_sha!r}"
    )


def test_validation_installs_the_packages_the_lane_built() -> None:
    """Feed the validator the artefacts, images, and names packaging produced.

    Every value here is read from the packaging step or from an earlier gate
    step rather than repeated, so a rename or a changed output cannot leave
    the validator looking at a package the lane never built.
    """
    packaging = _inputs(PACKAGING_STEP)
    validation = _inputs(VALIDATION_STEP)
    assert validation["formats"] == "deb,rpm", (
        f"both package formats must be installed, got {validation['formats']!r}"
    )
    assert validation["packages-dir"] == packaging["outdir"], (
        "validation must read the directory packaging wrote"
    )
    for key in ("package-name", "bin-name", "target"):
        assert validation[key] == packaging[key], (
            f"validation must reuse the packaging step's {key!r}, got "
            f"{validation[key]!r} against {packaging[key]!r}"
        )
    assert validation["version"] == "${{ steps.validation_version.outputs.version }}", (
        "validation must use the nfpm-normalised version, not the Cargo spelling"
    )
    assert validation["deb-base-image"] == (
        "${{ steps.validation_deb_image.outputs.image }}"
    ), "the Debian sandbox must be the image this lane built"
    assert validation["rpm-base-image"] == (
        "${{ steps.validation_rpm_image.outputs.image }}"
    ), "the RPM sandbox must be the image this lane built"
    assert MAN_PAGE_PATH in str(validation["expected-paths"]).split(), (
        f"the installed manual page {MAN_PAGE_PATH!r} must be an expected path"
    )
    verify_command = str(validation["verify-command"])
    assert verify_command.startswith("/usr/local/bin/verify-installed-package "), (
        "the sandbox must run the repository's verification script"
    )
    bin_name = packaging["bin-name"]
    assert isinstance(bin_name, str), (
        f"the packaging step must name the binary as a string, got {bin_name!r}"
    )
    assert bin_name in verify_command, (
        "the verification script must be given the packaged binary's name"
    )
    assert str(validation["polythene-path"]).endswith(
        "/.github/package-validation/polythene_cli.py"
    ), "the validator must launch the repository's pinned Polythene wrapper"


def test_cross_architecture_packages_are_installed_under_emulation() -> None:
    """Install the packages the shared action would skip on an arch mismatch.

    The action skips both its sandbox install and its architecture check when
    the package architecture differs from the runner's, which is the arm64 lane
    on an x64 release runner. Without this step those packages would reach
    upload having never run. It must therefore be unconditional within the
    cross-architecture lane, install both formats from the images this lane
    built, and hand each package to the verifier with its architecture.
    """
    step = named_step(_linux_steps(), EMULATION_STEP)
    env = require_mapping(step.get("env"), f"the {EMULATION_STEP} environment")
    expected_env = {
        "PACKAGE_ARCH": "${{ steps.validation_sandbox.outputs.arch }}",
        "DEB_IMAGE": "${{ steps.validation_deb_image.outputs.image }}",
        "RPM_IMAGE": "${{ steps.validation_rpm_image.outputs.image }}",
        "BIN_NAME": "${{ inputs['bin-name'] }}",
    }
    for key, value in expected_env.items():
        assert env.get(key) == value, (
            f"{EMULATION_STEP!r} must read {key} from {value!r}, got {env.get(key)!r}"
        )
    run = str(step["run"])
    fragments = (
        "debs=(dist/*.deb)",
        "rpms=(dist/*.rpm)",
        'packages=("${debs[@]}" "${rpms[@]}")',
        "*.deb) image=$DEB_IMAGE",
        "*.rpm) image=$RPM_IMAGE",
        "sh .github/package-validation/install-emulated-package.sh",
        '"$image" "$PACKAGE_ARCH" "$package" "$BIN_NAME"',
    )
    for fragment in fragments:
        assert fragment in run, (
            f"{EMULATION_STEP!r} must install every package with {fragment!r}, "
            f"got {run!r}"
        )


@pytest.mark.parametrize(
    ("cargo_version", "expected_version"),
    [
        ("0.1.0-beta4", "0.1.0~beta4"),
        ("0.1.0", "0.1.0"),
        ("v0.1.0-beta4", "0.1.0~beta4"),
    ],
)
def test_validation_version_matches_what_nfpm_writes(
    tmp_path: Path, cargo_version: str, expected_version: str
) -> None:
    """Hand the validator the version spelling nfpm puts in the packages.

    nfpm rewrites a hyphen in the version to ``~`` for both formats; the
    validator compares that metadata against the version it was given, so a
    prerelease tag validated with the Cargo spelling fails on every package
    even when the package is correct. A leading ``v`` from a Git tag is
    stripped for the same reason.
    """
    script = str(named_step(_linux_steps(), VERSION_STEP)["run"])
    output = tmp_path / "github-output"
    output.write_text("", encoding="utf-8")
    subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the step's own script is under test.
        ["/bin/bash", "-c", script],
        check=True,
        capture_output=True,
        text=True,
        env={
            "PATH": "/usr/bin:/bin",
            "PACKAGE_VERSION": cargo_version,
            "GITHUB_OUTPUT": str(output),
        },
    )
    assert output.read_text(encoding="utf-8") == f"version={expected_version}\n", (
        f"{cargo_version!r} must be handed to the validator as "
        f"{expected_version!r}, the spelling nfpm records"
    )


@pytest.mark.parametrize(
    ("step_name", "containerfile", "image_name"),
    [
        (
            DEB_IMAGE_STEP,
            ".github/package-validation/deb.Containerfile",
            "netsuke-package-validation-deb",
        ),
        (
            RPM_IMAGE_STEP,
            ".github/package-validation/rpm.Containerfile",
            "netsuke-package-validation-rpm",
        ),
    ],
)
def test_image_builds_use_the_target_architecture_and_stay_local(
    step_name: str, containerfile: str, image_name: str
) -> None:
    """Build each sandbox for the package's architecture under a local tag.

    ``--arch`` matters because the sandbox runs the package's own binaries, so
    an amd64 image cannot validate an arm64 package. The ``localhost/`` tag
    keeps the image out of a registry: the job is read-only and the image is
    consumed from the local Podman store by the validator.
    """
    step = named_step(_linux_steps(), step_name)
    run = str(step["run"])
    env = require_mapping(step.get("env"), f"the {step_name} environment")
    assert env.get("PACKAGE_ARCH") == (
        "${{ steps.validation_sandbox.outputs.arch }}"
    ), "the image must follow the architecture the sandbox step resolved"
    fragments = (
        f'image="localhost/{image_name}:${{PACKAGE_ARCH}}"',
        "podman build",
        '--arch "$PACKAGE_ARCH"',
        f"--file {containerfile}",
        '--tag "$image"',
        ".github/package-validation",
        'echo "image=$image" >> "$GITHUB_OUTPUT"',
    )
    for fragment in fragments:
        assert fragment in run, (
            f"{step_name!r} must build the image with {fragment!r}, got {run!r}"
        )


def test_validation_runs_in_the_release_dry_run() -> None:
    """Run the gate on pull requests as well as on tags.

    The release workflow is the only caller of the packaging workflow, so the
    gate reaches the dry run exactly when ``build-linux`` is unconditional
    there and the dry run calls the same workflow with ``dry-run: true``. The
    publishing job must also need ``build-linux``, or a broken package could
    be published before it was ever installed.
    """
    release = load_workflow(RELEASE_WORKFLOW_PATH)
    build_linux = workflow_job(release, "build-linux")
    assert build_linux.get("if") is None, (
        "the Linux packaging lane must not be skipped in a dry run"
    )
    assert build_linux.get("uses") == "./.github/workflows/build-and-package.yml", (
        "the Linux lane must call the packaging workflow this gate lives in"
    )
    with_inputs = require_mapping(build_linux.get("with"), "build-linux inputs")
    assert with_inputs.get("platform") == "linux", (
        "the Linux lane must select the Linux steps of the packaging workflow"
    )
    release_job = workflow_job(release, "release")
    needs = require_list(release_job.get("needs", []), "the publishing job's needs")
    assert "build-linux" in needs, (
        "publishing must wait for the Linux packaging lane's validation"
    )
    dry_run = load_workflow(REPO_ROOT / ".github" / "workflows" / "release-dry-run.yml")
    dry_run_inputs = require_mapping(
        workflow_job(dry_run, "release").get("with"), "the dry-run release inputs"
    )
    assert dry_run_inputs.get("dry-run") is True, (
        "the pull-request dry run must invoke the release workflow in dry-run mode"
    )


def test_validation_stays_off_the_registry() -> None:
    """Keep the packaging job read-only: the images are built, never pushed.

    Both images are consumed from the local Podman store, so the job needs no
    package-write scope. A later change that pushed them would have to widen
    this permission and update the contract deliberately rather than by
    accident.
    """
    build = workflow_job(load_workflow(PACKAGE_WORKFLOW_PATH), "build")
    assert build.get("permissions") == {"contents": "read"}, (
        "the packaging job must stay read-only; the validation images are "
        f"built locally rather than published, got {build.get('permissions')!r}"
    )
