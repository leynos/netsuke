"""Whole-workflow guard tests for the mutation caller's variable contract.

``setup-rust`` exports ``RUSTFLAGS`` in the worker, so the caller must not
assign it: Cargo takes an assigned value over every ``rustflags`` table in
``.cargo/config.toml``. The caller must not assign ``NETSUKE_PLATFORM_LINKER``
either, because that opt-out hands the worker the platform linker and undoes
the mold selection the configuration's ``linker`` key provides.

The guard walks the parsed workflow rather than the job's ``with:`` block, so a
future job, step, or reusable-workflow argument cannot introduce an override in
a corner the caller's exact-block assertion never reads.

Run via ``make test-workflow-contracts``.
"""

import copy

import pytest
from mutation_caller_variables import REPORTED_VARIABLES, flag_override_problems
from workflow_loading import (
    MUTATION_TESTING_WORKFLOW_PATH,
    load_workflow,
    require_mapping,
)


def test_no_flag_or_linker_opt_out_is_set_anywhere_in_the_caller() -> None:
    """The caller never assigns a flag variable or the linker opt-out.

    Setting ``RUSTFLAGS`` here — to "help" with the linker, or to override
    anything else — would displace the build standard rather than configure it,
    and setting ``NETSUKE_PLATFORM_LINKER`` would opt the worker out of the
    mold selection the configuration provides. Both prohibitions are asserted
    over the whole parsed workflow and not only over the job's ``with:``
    block. Printing the variables in ``setup-commands`` is unaffected: a
    ``printf`` reads them, it does not assign them.
    """
    problems = flag_override_problems(load_workflow(MUTATION_TESTING_WORKFLOW_PATH))
    assert not problems, (
        "the caller must not set a compiler-flag variable or the linker "
        "opt-out anywhere; Cargo takes an assigned RUSTFLAGS over the "
        "project's rustflags table, and the worker must select mold; "
        f"found {problems}"
    )


@pytest.mark.parametrize(
    "variable",
    [pytest.param(name, id=name.lower()) for name in REPORTED_VARIABLES],
)
def test_flag_guard_rejects_a_seeded_override(variable: str) -> None:
    """The guard fails on a caller that sets any forbidden variable.

    This is the ExecPlan's promised negative control, kept as a test rather
    than a one-off manual edit so it stays runnable: a guard that cannot fail
    on the defect it names is not a guard. The fixture mirrors the shape the
    real caller could grow — an ``env:`` block on the calling job — and the
    control would also catch it at workflow, step, or argument scope.
    """
    workflow = load_workflow(MUTATION_TESTING_WORKFLOW_PATH)
    seeded = copy.deepcopy(workflow)
    job = require_mapping(
        require_mapping(seeded.get("jobs"), "jobs")["mutation"], "jobs.mutation"
    )
    job["env"] = {variable: "seeded"}

    assert flag_override_problems(seeded), (
        f"the guard must reject a caller that seeds an env.{variable} "
        "override; it accepted one"
    )
    assert not flag_override_problems(workflow), (
        "the unmodified caller must satisfy the guard"
    )
