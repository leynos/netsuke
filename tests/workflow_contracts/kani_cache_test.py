"""Hold the Kani smoke job's shared-installer and cache contract.

The pinned ``rust-prover-tools`` revision owns Kani's verified prebuilt
archives. The job keeps its Cargo front-end, Kani files and supporting Rust
toolchain inside three cached workspace homes, publishes the front-end to
following steps, and uses a new cache generation for the changed layout. These
checks keep CI on that shared path and prevent stale binaries or a source build.

Run via ``make test-workflow-contracts``.
"""

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


PROVER_TOOLS_SOURCE_PIN = "592558f7183b554fc749696246984c5d7300d029"
PROVER_TOOLS_SOURCE = (
    "git+https://github.com/leynos/rust-prover-tools@" + PROVER_TOOLS_SOURCE_PIN
)
MAKEFILE = REPO_ROOT / "Makefile"


def test_kani_uses_pinned_shared_installer_and_cached_paths() -> None:
    """Require CI to install Kani through the pinned shared tool."""
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
    assert steps.index(named_step(steps, "Setup uv")) < steps.index(install_step), (
        "uv must be set up before the shared installer runs through it"
    )
    install_command = str(install_step.get("run", ""))
    expected_install_command = (
        REPO_ROOT
        / "tests"
        / "workflow_contracts"
        / "snapshots"
        / "kani_install_command.sh"
    ).read_text(encoding="utf-8")
    assert install_command == expected_install_command, (
        "the Kani install step must match the reviewed shared-tool invocation"
    )
    install_env = require_mapping(install_step.get("env", {}), "Kani install step env")
    assert not {
        "INPUT_KANI_VERSION",
        "INPUT_FRONTEND_SHA256",
        "INPUT_BUNDLE_SHA256",
    }.intersection(install_env), "the old script bindings must be removed"
    assert not (REPO_ROOT / "scripts" / "ci" / "install_kani.py").exists(), (
        "the bespoke Kani CI downloader should be removed"
    )
    assert PROVER_TOOLS_SOURCE in MAKEFILE.read_text(encoding="utf-8"), (
        "Makefile must pin rust-prover-tools to the inspected upstream main head"
    )
    assert "2ba17da6b7e24160c4041e8d41ba1ba6cca87b35" not in MAKEFILE.read_text(
        encoding="utf-8"
    ), "Makefile must not retain the old rust-prover-tools revision"
    assert str(named_step(steps, "Kani version check").get("run", "")).strip() == (
        "make kani-check"
    ), "the Kani smoke lane must use the shared version check"
    cache_action = KANI_CACHE_ACTION.read_text(encoding="utf-8")
    assert "netsuke-kani-v2-" in cache_action, (
        "the Kani cache key must use the new installation generation"
    )
    assert "netsuke-kani-v1-" not in cache_action, (
        "the Kani cache key must not reuse the old installation generation"
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
    for path in sorted({*WORKFLOW_DIR.glob("*.yml"), *WORKFLOW_DIR.glob("*.yaml")}):
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
