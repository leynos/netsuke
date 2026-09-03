"""Hold the Kani smoke job's binary-only, cache-aware installation contract.

Kani ships as two payloads: a Cargo front-end and a verifier bundle. The job
installs both from pinned archives with published checksums, keeps them under
version-qualified directories on the Namespace cache volume, and never
compiles the verifier from source. These checks pin that arrangement so a
workflow edit cannot quietly reintroduce a source build or a stale binary.

Run via ``make test-workflow-contracts``.
"""

from workflow_loading import (
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
)

#: Job-local homes Kani redirects into the workspace, and the reason for each.
KANI_HOMES = {
    "CARGO_HOME": (
        ".kani-cargo",
        "Kani's Cargo front-end must live in the cached job-local Cargo home",
    ),
    "KANI_HOME": (
        ".kani-home",
        "Kani's verifier payload must live in the cached job-local Kani home",
    ),
    "RUSTUP_HOME": (
        ".kani-rustup",
        "Kani's supporting toolchain must live in the cached job-local Rustup home",
    ),
}


def _assert_kani_homes_are_cached(workflow: dict[str, object]) -> None:
    """Require each redirected Kani home to be job-local and on the volume."""
    job = require_mapping(
        require_mapping(workflow.get("jobs"), "workflow jobs")["kani-smoke"],
        "kani-smoke",
    )
    env = require_mapping(job.get("env"), "kani-smoke env")
    cache_inputs = require_mapping(
        named_step(job_steps(workflow, "kani-smoke"), "Set up Kani cache volume").get(
            "with"
        ),
        "Kani cache inputs",
    )
    cached_paths = str(cache_inputs.get("path", ""))
    for name, (directory, reason) in KANI_HOMES.items():
        assert env.get(name) == f"${{{{ github.workspace }}}}/{directory}", reason
        assert directory in cached_paths, f"Kani must cache {directory}"


def test_kani_uses_cached_prebuilt_frontend_and_release_bundle() -> None:
    """Require Kani's separate front-end and verifier payloads to be cacheable."""
    workflow = load_workflow()
    _assert_kani_homes_are_cached(workflow)

    steps = job_steps(workflow, "kani-smoke")
    setup_inputs = require_mapping(
        named_step(steps, "Setup Rust").get("with"), "Setup Rust inputs"
    )
    assert setup_inputs.get("install-binstall") == "false", (
        "Kani installs its cached front-end directly from a pinned binary archive"
    )

    install_step = named_step(steps, "Install prebuilt Kani")
    assert steps.index(named_step(steps, "Set up Kani cache volume")) < steps.index(
        install_step
    ), "the Kani cache volume must be mounted before Kani is installed"

    install_command = str(install_step.get("run"))
    required_install_fragments = (
        "quickinstall='https://github.com/cargo-bins/cargo-quickinstall'",
        '"${quickinstall}/releases/download/kani-verifier-${kani_version}/',
        "ed2bafc239b834e14c6b66fc4838e342",
        "upstream='https://github.com/model-checking/kani'",
        '"${upstream}/releases/download/kani-${kani_version}/',
        "3b5f7afd3b51603ee720db7bc1bc4fe4",
        'frontend_bin="${CARGO_HOME}/frontend/kani-${kani_version}"',
        'kani_dir="${KANI_HOME}/kani-${kani_version}"',
        '[[ ! -x "${frontend_bin}/cargo-kani"',
        '[[ ! -x "${kani_dir}/bin/kani-driver"',
        'cargo kani setup --use-local-bundle "${bundle}"',
    )
    missing_fragments = tuple(
        fragment
        for fragment in required_install_fragments
        if fragment not in install_command
    )
    assert not missing_fragments, (
        f"Kani's binary-only cached installation is missing {missing_fragments!r}"
    )
    _assert_kani_archives_are_verified_before_use(install_command)


def _assert_kani_archives_are_verified_before_use(install_command: str) -> None:
    """Require each Kani archive's checksum to gate its own unpacking step.

    A bare `sha256sum --check` substring would also pass if the workflow
    verified an unrelated file, so each assertion names the archive variable
    and requires the verification to precede that archive's extraction.
    """
    frontend_check = '"${frontend_archive}" | sha256sum --check --'
    frontend_extract = 'tar --extract --gzip --file "${frontend_archive}"'
    bundle_check = '"${bundle}" | sha256sum --check --'
    bundle_use = 'cargo kani setup --use-local-bundle "${bundle}"'
    for check, use, label in (
        (frontend_check, frontend_extract, "front-end archive"),
        (bundle_check, bundle_use, "verifier bundle"),
    ):
        assert check in install_command, (
            f"the Kani {label} must be checksum-verified by name"
        )
        assert use in install_command, f"the Kani {label} must be unpacked by name"
        assert install_command.index(check) < install_command.index(use), (
            f"the Kani {label} must be verified before it is unpacked"
        )
