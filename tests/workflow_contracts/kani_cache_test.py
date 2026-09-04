"""Hold the Kani smoke job's binary-only, cache-aware installation contract.

Kani ships as two payloads: a Cargo front-end and a verifier bundle. The job
installs both from pinned archives with published checksums, keeps them under
version-qualified directories inside the three cached workspace homes, and
never compiles the verifier from source. These checks pin that arrangement so
a workflow edit cannot quietly reintroduce a source build or a stale binary.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import yaml
from workflow_loading import (
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
    require_list,
    require_mapping,
    workflow_job,
)

KANI_CACHE_ACTION = REPO_ROOT / ".github" / "actions" / "kani-cache" / "action.yml"
WORKFLOW_DIR = REPO_ROOT / ".github" / "workflows"


def _action_steps() -> list[dict[str, object]]:
    """Return the Kani cache action's composite steps, in order."""
    document = require_mapping(
        yaml.safe_load(KANI_CACHE_ACTION.read_text(encoding="utf-8")), "kani-cache"
    )
    runs = require_mapping(document.get("runs"), "runs")
    return [
        require_mapping(step, "kani-cache step")
        for step in require_list(runs.get("steps"), "kani-cache steps")
    ]


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


def _cache_action_paths() -> set[str]:
    """Return every path the Kani cache action claims, across both modes."""
    steps = _action_steps()
    return {
        line.strip()
        for step in steps
        if "/cache/" in str(step.get("uses", ""))
        for line in str(
            require_mapping(step.get("with"), "cache inputs")["path"]
        ).splitlines()
        if line.strip()
    }


def _assert_kani_homes_are_cached(workflow: dict[str, object]) -> None:
    """Require each redirected Kani home to be job-local and cached."""
    env = require_mapping(
        workflow_job(workflow, "kani-smoke").get("env"), "kani-smoke env"
    )
    cached_paths = _cache_action_paths()
    for name, (directory, reason) in KANI_HOMES.items():
        assert env.get(name) == f"${{{{ github.workspace }}}}/{directory}", reason
        assert directory in cached_paths, f"Kani must cache {directory}"


def test_kani_payloads_share_one_versioned_cache_entry() -> None:
    """Require the three Kani homes to be warm or cold together.

    A multi-part tool is a multi-part contract: restoring the front-end
    without the verifier bundle leaves `cargo kani` present and unusable, so
    all three directories share one entry keyed by the pinned version.
    """
    steps = _action_steps()
    key_script = str(
        next(step for step in steps if step.get("id") == "keys").get("run", "")
    )
    assert "tools/kani/VERSION" in key_script, (
        "the Kani cache key must be derived from the pinned version file"
    )
    cache_steps = [step for step in steps if "/cache/" in str(step.get("uses", ""))]
    assert len(cache_steps) == 2, (
        f"the Kani cache action must declare one restore and one save, "
        f"got {cache_steps!r}"
    )
    paths = {
        str(require_mapping(step.get("with"), "cache inputs")["path"])
        for step in cache_steps
    }
    assert len(paths) == 1, f"restore and save must claim the same paths: {paths!r}"


class _InstallFragment(typ.NamedTuple):
    """A required substring of the install script, and the concern it protects."""

    concern: str
    text: str


#: Required substrings of the install script, one row per concern so a
#: regression names the property it broke rather than a bare fragment. Each
#: concern groups the URLs, checksum prefixes, directory layout, or
#: executable probes that together protect one part of the binary-only,
#: cache-aware contract.
REQUIRED_INSTALL_FRAGMENTS = (
    _InstallFragment(
        "quickinstall front-end download host",
        "quickinstall='https://github.com/cargo-bins/cargo-quickinstall'",
    ),
    _InstallFragment(
        "quickinstall front-end archive URL",
        '"${quickinstall}/releases/download/kani-verifier-${kani_version}/',
    ),
    _InstallFragment(
        "quickinstall front-end archive checksum prefix",
        "ed2bafc239b834e14c6b66fc4838e342",
    ),
    _InstallFragment(
        "upstream verifier download host",
        "upstream='https://github.com/model-checking/kani'",
    ),
    _InstallFragment(
        "upstream verifier bundle URL",
        '"${upstream}/releases/download/kani-${kani_version}/',
    ),
    _InstallFragment(
        "upstream verifier bundle checksum prefix",
        "3b5f7afd3b51603ee720db7bc1bc4fe4",
    ),
    _InstallFragment(
        "front-end directory layout",
        'frontend_bin="${CARGO_HOME}/frontend/kani-${kani_version}"',
    ),
    _InstallFragment(
        "verifier directory layout",
        'kani_dir="${KANI_HOME}/kani-${kani_version}"',
    ),
    _InstallFragment(
        "front-end executable probe",
        '[[ ! -x "${frontend_bin}/cargo-kani"',
    ),
    _InstallFragment(
        "verifier executable probe",
        '[[ ! -x "${kani_dir}/bin/kani-driver"',
    ),
    _InstallFragment(
        "local bundle setup invocation",
        'cargo kani setup --use-local-bundle "${bundle}"',
    ),
)


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
    assert steps.index(named_step(steps, "Restore Kani payloads")) < steps.index(
        install_step
    ), "the Kani cache must be restored before Kani is installed"

    install_command = str(install_step.get("run"))
    missing_fragments = [
        fragment
        for fragment in REQUIRED_INSTALL_FRAGMENTS
        if fragment.text not in install_command
    ]
    assert not missing_fragments, (
        "Kani's binary-only cached installation is missing: "
        + ", ".join(
            f"{fragment.concern} ({fragment.text!r})" for fragment in missing_fragments
        )
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


def test_kani_cache_action_requires_runner_image() -> None:
    """Require the kani-cache action to declare `runner-image` as required.

    The key renderer reads `NETSUKE_RUNNER_IMAGE` under `set -u`; an
    undeclared input would let a future caller omit it and abort the job
    while rendering the cache key, instead of failing with a clear
    missing-input error before the job runs.
    """
    document = require_mapping(
        yaml.safe_load(KANI_CACHE_ACTION.read_text(encoding="utf-8")), "kani-cache"
    )
    inputs = require_mapping(document.get("inputs"), "kani-cache inputs")
    runner_image = require_mapping(inputs.get("runner-image"), "runner-image input")
    assert runner_image.get("required") is True, (
        "the kani-cache action must declare runner-image as a required input"
    )


def _kani_cache_steps(
    workflow: dict[str, object], workflow_name: str
) -> list[tuple[str, dict[str, object]]]:
    """Return every (job name, step) pair that calls the kani-cache action."""
    jobs = require_mapping(workflow.get("jobs"), f"{workflow_name} jobs")
    pairs: list[tuple[str, dict[str, object]]] = []
    for job_name, declaration in jobs.items():
        job = require_mapping(declaration, f"{workflow_name} {job_name}")
        if "steps" not in job:
            # A job calling a reusable workflow declares no steps of its own,
            # so it cannot call the local kani-cache action directly.
            continue
        pairs.extend(
            (job_name, step)
            for step in job_steps(workflow, job_name)
            if step.get("uses") == "./.github/actions/kani-cache"
        )
    return pairs


def test_every_kani_cache_caller_supplies_runner_image() -> None:
    """Require every kani-cache step to pass a non-empty `runner-image`.

    A caller that stops forwarding the workflow-level runner image would
    otherwise only fail deep inside the composite action's own `set -u`
    boundary while rendering the cache key, rather than at the call site.
    """
    offenders: list[str] = []
    for path in sorted(WORKFLOW_DIR.glob("*.yml")):
        workflow = load_workflow(path)
        for job_name, step in _kani_cache_steps(workflow, path.name):
            inputs = step.get("with")
            runner_image = (
                inputs.get("runner-image") if isinstance(inputs, dict) else None
            )
            if not isinstance(runner_image, str) or not runner_image.strip():
                offenders.append(f"{path.name} {job_name} {step.get('name')!r}")
    assert not offenders, (
        f"every kani-cache step must supply a non-empty runner-image: {offenders!r}"
    )
