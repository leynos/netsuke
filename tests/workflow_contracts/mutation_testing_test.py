"""Contract tests for the mutation-testing caller workflow.

The executable logic lives in the ``leynos/shared-actions`` reusable
workflow, which carries its own unit and integration tests; netsuke's
caller is declarative configuration. These tests parse the caller through
the shared workflow loader and pin the contract it must uphold, so drift
(repointing the reference at a branch, widening permissions, or losing the
Kani-module excludes, the feature args, or the mold provisioning) fails CI
on the pull request rather than surfacing in a scheduled or manual run.

The caller must reference the correct reusable workflow at a commit SHA;
Dependabot owns the SHA value, so this test asserts the shape of the pin
(a 40-character lowercase hex commit SHA) rather than a specific SHA.

Two of the caller's inputs provision the build toolchain rather than the
mutation run, and they are asserted as a pair. ``install-mold`` asks the
shared ``setup-rust`` action to put the pinned mold on ``PATH``, which is
what the repository's own ``check-build-tools`` preflight requires before
any build target runs. ``setup-commands`` then re-runs that preflight
inside the worker, so a run whose mold is missing, unrunnable, or at the
wrong version says so before the baseline compiles rather than failing
later as an opaque linker error. Installing a linker on some other CI job
provisions that job's worker and not this one, which is why the input is
asserted here rather than assumed from ``ci.yml``.

The two assertions are deliberately independent of each other and of the
version. ``install-mold`` is pinned as the *string* ``"true"`` because the
shared workflow declares it ``type: string``; passing a YAML boolean would
be a type error at workflow-call time. No mold version is passed, because
the shared revision declares none and the repository's pin lives in
``tools/mold/VERSION``, which the preflight checks at run time.

Run via ``make test-workflow-contracts``.
"""

import pytest
from workflow_loading import (
    MUTATION_TESTING_WORKFLOW_PATH,
    load_workflow,
    require_mapping,
)

EXPECTED_USES_PATH = "leynos/shared-actions/.github/workflows/mutation-cargo.yml"

#: Kani-only verification modules gated behind ``#[cfg(kani)]`` mod
#: declarations; cargo-mutants does not evaluate that cfg, so their
#: survivors would be noise.
KANI_EXCLUDES = (
    "src/ir/cycle/verification.rs",
    "src/ir/cmd_interpolate/verification.rs",
    "src/ir/from_manifest/verification.rs",
    "src/ir/graph/kani_map.rs",
)

#: The shared workflow input that installs the pinned linker.
INSTALL_MOLD_INPUT = "install-mold"

#: The shared workflow input that runs caller commands before cargo-mutants.
SETUP_COMMANDS_INPUT = "setup-commands"

#: The repository target that verifies every build-tool pin.
BUILD_TOOLS_TARGET = "make check-build-tools"

#: The flag names the setup commands print for the run log.
FLAG_VARIABLES = ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")


# Both variables are printed because Cargo gives `CARGO_ENCODED_RUSTFLAGS`
# precedence and falls back to `RUSTFLAGS` only when the encoded form is unset,
# so a run where just one is set is visible in the log. An unset variable is
# not an error: the shared workflow's guide documents that `setup-rust` may
# leave `RUSTFLAGS` such that the project's own Cargo configuration decides the
# linker.
def _flag_report_lines() -> str:
    """Return one setup command per linker-flag variable, for the run log."""
    return "".join(
        f'printf "{name}=%s\\n" "${{{name}-<unset>}}"\n' for name in FLAG_VARIABLES
    )


#: The exact caller configuration; assert the whole block so an added or
#: dropped input fails loudly.
EXPECTED_WITH = {
    "exclude-globs": ",".join(KANI_EXCLUDES),
    "extra-args": "--all-features",
    INSTALL_MOLD_INPUT: "true",
    SETUP_COMMANDS_INPUT: BUILD_TOOLS_TARGET + "\n" + _flag_report_lines(),
}


def _triggers(workflow: dict[str, object]) -> dict[str, object]:
    """Return the workflow's ``on:`` mapping."""
    return require_mapping(workflow.get("on"), "the workflow on mapping")


def _mutation_job(workflow: dict[str, object]) -> dict[str, object]:
    """Return the single calling job."""
    jobs = require_mapping(workflow.get("jobs"), "the workflow jobs mapping")
    assert jobs, "the workflow must declare at least one job"
    assert list(jobs) == ["mutation"], (
        f"expected a single job named 'mutation', found {sorted(jobs)}"
    )
    return require_mapping(jobs["mutation"], "jobs.mutation")


def test_uses_reference_is_pinned_to_a_commit_sha() -> None:
    """The job must call mutation-cargo.yml, pinned to a full commit SHA.

    Dependabot owns the SHA value, so this asserts the shape of the pin
    (the correct reusable-workflow path, at a 40-character lowercase hex
    commit SHA) rather than a specific commit.
    """
    uses = _mutation_job(load_workflow(MUTATION_TESTING_WORKFLOW_PATH)).get("uses")
    assert isinstance(uses, str), f"jobs.mutation.uses must be a string, got {uses!r}"
    path, _, ref = uses.partition("@")
    assert path == EXPECTED_USES_PATH, (
        f"jobs.mutation.uses must reference mutation-cargo.yml, got {path!r}"
    )
    assert len(ref) == 40, (
        f"jobs.mutation.uses must pin a full 40-character commit SHA, "
        f"not a branch or tag: {ref!r}"
    )
    assert all(c in "0123456789abcdef" for c in ref), (
        f"jobs.mutation.uses must pin a lowercase hex commit SHA, "
        f"not a branch or tag: {ref!r}"
    )


def test_job_permissions_are_exactly_least_privilege() -> None:
    """The job grants contents: read and id-token: write, nothing broader."""
    permissions = _mutation_job(load_workflow(MUTATION_TESTING_WORKFLOW_PATH)).get(
        "permissions"
    )
    assert permissions == {"contents": "read", "id-token": "write"}, (
        "jobs.mutation.permissions must be exactly "
        f"{{'contents': 'read', 'id-token': 'write'}}, got {permissions!r}"
    )


def test_workflow_default_permissions_are_empty() -> None:
    """The workflow-level default token scope is empty."""
    workflow = load_workflow(MUTATION_TESTING_WORKFLOW_PATH)
    assert workflow.get("permissions") == {}, (
        f"top-level permissions must be an empty mapping, got "
        f"{workflow.get('permissions')!r}"
    )


def test_concurrency_serializes_per_ref_without_cancelling() -> None:
    """Runs queue per ref instead of cancelling one another."""
    concurrency = require_mapping(
        load_workflow(MUTATION_TESTING_WORKFLOW_PATH).get("concurrency"),
        "the workflow concurrency mapping",
    )
    assert concurrency.get("group") == "mutation-testing-${{ github.ref }}", (
        f"concurrency.group must key on the triggering ref, got "
        f"{concurrency.get('group')!r}"
    )
    assert concurrency.get("cancel-in-progress") is False, (
        f"concurrency.cancel-in-progress must be false, got "
        f"{concurrency.get('cancel-in-progress')!r}"
    )


def test_triggers_keep_schedule_and_plain_dispatch() -> None:
    """The daily schedule stays; dispatch has no legacy branch input."""
    triggers = _triggers(load_workflow(MUTATION_TESTING_WORKFLOW_PATH))
    schedule = triggers.get("schedule")
    assert schedule == [{"cron": "5 3 * * *"}], (
        f"on.schedule must be the daily 03:05 UTC cron, got {schedule!r}"
    )
    assert "workflow_dispatch" in triggers, "on.workflow_dispatch is missing"
    # A bodiless ``workflow_dispatch:`` parses as ``None``, which is a valid
    # trigger declaring no inputs; only a mapping can carry an input table.
    inputs: dict[str, object] = {}
    match triggers.get("workflow_dispatch"):
        case {"inputs": dict() as declared}:
            inputs = declared
    assert "branch" not in inputs, (
        "on.workflow_dispatch must not declare a branch input; the Actions "
        "run-workflow control selects the ref"
    )


def _mutation_with_block() -> dict[str, object]:
    """Return the caller's ``with:`` block."""
    return require_mapping(
        _mutation_job(load_workflow(MUTATION_TESTING_WORKFLOW_PATH)).get("with"),
        "jobs.mutation.with",
    )


def test_with_block_carries_the_caller_configuration() -> None:
    """The caller passes exactly the documented configuration."""
    with_block = _mutation_with_block()
    assert with_block == EXPECTED_WITH, (
        "jobs.mutation.with must be exactly the documented configuration "
        "(the #[cfg(kani)] module excludes, --all-features to match the CI "
        "baseline, install-mold so the worker has the pinned linker the "
        "check-build-tools preflight requires, and setup-commands to run "
        f"that preflight before the baseline); expected "
        f"{EXPECTED_WITH!r}, got {with_block!r}"
    )


def test_install_mold_is_enabled_in_the_type_the_shared_workflow_declares() -> None:
    """Mold provisioning is enabled, as the string the input's type requires.

    The shared workflow declares ``install-mold`` with ``type: string``. A
    bare YAML ``true`` would parse to the boolean ``True`` and the call
    would be rejected, so the value's type is asserted and not just its
    truthiness.
    """
    match _mutation_with_block().get(INSTALL_MOLD_INPUT):
        case str() as value:
            pass
        case other:
            pytest.fail(
                f"jobs.mutation.with.{INSTALL_MOLD_INPUT} must be the string "
                f"'true'; the shared workflow declares it type: string, and a "
                f"YAML boolean would be rejected at workflow-call time. Got "
                f"{other!r} ({type(other).__name__})"
            )
    assert value == "true", (
        f"jobs.mutation.with.{INSTALL_MOLD_INPUT} must enable the pinned "
        f"mold install, got {value!r}"
    )


def test_setup_commands_verify_the_build_tool_pins_before_the_baseline() -> None:
    """The worker re-runs the repository preflight, then prints the flags.

    Asserting the preflight rather than a bare version string ties the
    caller to the repository's own definition of "the pinned toolchain is
    present": ``check-build-tools`` reads ``tools/mold/VERSION`` itself, so
    a mold bump cannot leave this caller asserting a stale number. The
    printed variables are what the run log carries for the open question
    about which flag source selects the linker.
    """
    match _mutation_with_block().get(SETUP_COMMANDS_INPUT):
        case str() as value:
            pass
        case other:
            pytest.fail(
                f"jobs.mutation.with.{SETUP_COMMANDS_INPUT} must be a string, "
                f"got {other!r} ({type(other).__name__})"
            )
    assert BUILD_TOOLS_TARGET in value, (
        f"jobs.mutation.with.{SETUP_COMMANDS_INPUT} must run "
        f"{BUILD_TOOLS_TARGET!r} so a missing or drifted tool stops the run "
        f"before the baseline compiles; got {value!r}"
    )
    for name in FLAG_VARIABLES:
        assert name in value, (
            f"jobs.mutation.with.{SETUP_COMMANDS_INPUT} must report {name} "
            f"in the run log; got {value!r}"
        )
