"""Hold the contract for publishing sccache's Actions cache credentials.

Where the export sits, which runs it belongs to, and which endpoint it names.
A server started without these variables stays in local-disk mode for the whole
job and reports a plausible hit rate on the second half of its own
compilation, so every one of these is a silent failure rather than a loud one.

Split from `sccache_contract_test.py`, which holds the separate question of
whether a job has a wrapper, exactly one backend, and statistics around its
compile steps.

Run via ``make test-workflow-contracts``.
"""

import pytest
from cache_contract_data import (
    ACTION_DIR,
    SCCACHE_CREDENTIAL_JOBS,
    SCCACHE_CREDENTIALS_ACTION,
    SETUP_RUST_ACTION,
    WORKFLOW_DIR,
    lane_steps,
)
from fork_fallback import FORK_FALLBACK_KEYS, FORK_GUARD
from workflow_loading import job_steps, load_workflow, require_mapping

#: The condition that selects this repository's own runs. A fork's pull
#: request sets the field to `true`; a push and this repository's own pull
#: request leave it `false` or null, and `!= true` is how one expression covers
#: both without a second clause.
OWNED_ARM_GUARD = f"{FORK_GUARD} != true"


@pytest.mark.parametrize(("workflow_name", "job_name"), SCCACHE_CREDENTIAL_JOBS)
def test_credentials_are_exported_before_the_server_can_start(
    workflow_name: str, job_name: str
) -> None:
    """Require the credential export before anything starts sccache.

    A `run` step on Ubicloud cannot see `ACTIONS_RESULTS_URL` or
    `ACTIONS_RUNTIME_TOKEN`, and the shared Rust setup action that normally
    publishes them is disabled here. `sccache --zero-stats`, `--start-server`,
    and the first wrapped `rustc` each start the server, and a server started
    without those variables stays in local-disk mode for the whole job and
    reports zero compile requests.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    export_indices = [
        index
        for index, step in enumerate(steps)
        if str(step.get("uses", "")) == SCCACHE_CREDENTIALS_ACTION
    ]
    assert len(export_indices) == 1, (
        f"{workflow_name} {job_name} must export sccache credentials exactly "
        f"once, got {export_indices!r}"
    )
    export_index = export_indices[0]
    checkout_indices = [
        index
        for index, step in enumerate(steps)
        if "actions/checkout@" in str(step.get("uses", ""))
    ]
    assert checkout_indices, f"{workflow_name} {job_name} must check out first"
    assert export_index == checkout_indices[0] + 1, (
        f"{workflow_name} {job_name} must export credentials immediately after checkout"
    )
    starters = [
        index
        for index, step in enumerate(steps)
        if str(step.get("name", "")) in {"Install sccache", "Reset sccache statistics"}
        or "sccache" in str(step.get("run", ""))
    ]
    assert starters, f"{workflow_name} {job_name} should touch sccache somewhere"
    assert export_index < min(starters), (
        f"{workflow_name} {job_name} must export credentials before anything "
        "can start the sccache server"
    )


@pytest.mark.parametrize(("workflow_name", "job_name"), SCCACHE_CREDENTIAL_JOBS)
def test_the_credential_export_runs_on_the_owned_arm_alone(
    workflow_name: str, job_name: str
) -> None:
    """The export belongs to the Ubicloud arm and to nothing else.

    The action clears sccache's v2 switch and publishes Ubicloud's proxy
    address. A fork's pull request runs the same lane on a GitHub-hosted
    runner, where that address is GitHub's own or empty, and the action's own
    verification step then fails the job outright because
    `SCCACHE_GHA_ENABLED` is `true`. The hosted arm keeps GitHub's native cache
    configuration instead, which sccache reads for itself.

    The guard is asserted as the fork field by name. `private` and `archived`
    sit in the same position, parse identically and evaluate, and either would
    send every run down one branch while the declaration still looked right.

    It is also asserted to be satisfiable: a guard that never holds would
    switch the export off for this repository's own branches too, which is the
    failure the export exists to prevent, and every other assertion about the
    export would go on passing.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    export = next(
        step
        for step in steps
        if str(step.get("uses", "")) == SCCACHE_CREDENTIALS_ACTION
    )
    condition = str(export.get("if", "")).strip()
    key = f"{workflow_name.removesuffix('.yml')}.{job_name}"
    if key not in FORK_FALLBACK_KEYS:
        # No fork reaches this lane, so it has one arm and the export is
        # unconditional. A guard here would be worse than absent: it would
        # switch the export off on the only runs the lane has, and the job
        # would pass with the server on local disk.
        assert not condition, (
            f"{workflow_name} {job_name} has no fork arm, so its credential "
            f"export must be unconditional; it declares {condition!r}"
        )
        return
    assert condition == OWNED_ARM_GUARD, (
        f"{workflow_name} {job_name} must guard the credential export on "
        f"{OWNED_ARM_GUARD!r}, not {condition!r}: the action is correct only "
        f"on the Ubicloud arm, and a sibling field of the same shape would "
        f"send every run down one branch"
    )
    assert FORK_GUARD in condition, (
        f"{workflow_name} {job_name} must name the fork field itself"
    )
    assert "!= true" in condition, (
        f"{workflow_name} {job_name} must select the non-fork case; a guard "
        f"that never holds disables the export on this repository's own "
        f"branches while every other assertion about it still passes"
    )


def test_the_export_names_the_proxy_endpoint_not_the_results_service() -> None:
    """Require the export to point sccache at the endpoint Ubicloud serves.

    Ubicloud intercepts the cache service with a local proxy advertised as
    `ACTIONS_CACHE_URL`, which serves the v1 API. sccache 0.16 prefers
    GitHub's v2 results service whenever `ACTIONS_CACHE_SERVICE_V2` is set,
    and that address resolves past the proxy to GitHub. Exporting
    `ACTIONS_RESULTS_URL` did exactly that: 5310 requests, zero hits, and one
    write error per miss, with every object landing in GitHub's store.
    """
    steps = lane_steps(ACTION_DIR / "sccache-gha-credentials" / "action.yml", None)
    script = str(
        require_mapping(steps[0].get("with"), "export inputs").get("script", "")
    )
    required = {
        "ACTIONS_CACHE_URL": "publish the proxy address sccache should use",
        "ACTIONS_RUNTIME_TOKEN": "publish the token that address requires",
        "ACTIONS_CACHE_SERVICE_V2', ''": (
            "clear the v2 switch, which routes past the proxy"
        ),
    }
    missing = [reason for token, reason in required.items() if token not in script]
    assert not missing, f"the export must {'; '.join(missing)}"
    assert "ACTIONS_RESULTS_URL" not in script, (
        "the export must not publish the v2 results service address, which is "
        "what sent 92 sccache objects to GitHub instead of Ubicloud"
    )


@pytest.mark.parametrize(("workflow_name", "job_name"), SCCACHE_CREDENTIAL_JOBS)
def test_the_export_precedes_setup_rust_and_the_server_start(
    workflow_name: str, job_name: str
) -> None:
    """Require the export before setup-rust and before any server start.

    On Ubicloud the runner re-injects the v2 service variables into every
    action step, so a server started inside `setup-rust` binds GitHub's
    service whatever the export said. Every job here passes
    `use-sccache: false` and starts the server from a `run` step after the
    export instead.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    export = next(
        index
        for index, step in enumerate(steps)
        if str(step.get("uses", "")) == SCCACHE_CREDENTIALS_ACTION
    )
    setup = next(
        index
        for index, step in enumerate(steps)
        if SETUP_RUST_ACTION in str(step.get("uses", ""))
    )
    assert export < setup, (
        f"{workflow_name} {job_name} must export before the toolchain setup"
    )
    inputs = require_mapping(steps[setup].get("with"), "Setup Rust inputs")
    assert inputs.get("use-sccache") == "false", (
        f"{workflow_name} {job_name} must not let setup-rust start the server"
    )
    starts = [
        index
        for index, step in enumerate(steps)
        if "sccache --zero-stats" in str(step.get("run", ""))
    ]
    for start in starts:
        assert export < start, (
            f"{workflow_name} {job_name} must export before starting the server"
        )
