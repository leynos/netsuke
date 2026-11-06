"""Hold the Kani lane's mutation compile gate to the workflow that runs it.

`make test-kani-mutations` first warms the Kani target in one Nextest invocation,
then compiles each mutation patch's patched tree in a second invocation. The
Make target is not enough on its
own: it can be perfectly written and never invoked, and
`tests/makefile_test_target.rs` pins its recipe without pinning the CI wiring.

The gap is specific. `tests/workflow_ci.rs` asserts that the `kani-smoke` job
runs `make kani-ir`, caches its payloads, and carries a raised
`timeout-minutes`, but nothing asserts the mutation gate step at all. Delete
`run: make test-kani-mutations` from that job and every contract still passes,
the job stays green, and the gate silently stops running -- issue #756's own
failure mode (a check that looks healthy while proving nothing), one level up.

Ordering is asserted, not merely presence. The gate compiles through the Kani
frontend the earlier steps install, needs the mold linker and pinned toolchain
that its own `check-build-tools` prerequisite verifies, and runs under
`cargo-nextest`. A gate hoisted above any of those is still present, and still
cannot run; only the assertion on position catches that.

What this module deliberately does not re-assert is the gate's own guard. Its
`#[ignore]` attribute, its `--run-ignored ignored-only` selection, and its
`GATE_RUSTFLAGS` composition are the Make target's contract, already held by
`tests/makefile_test_target.rs`. The seam left uncovered was the wiring.

Run via ``make test-workflow-contracts``.
"""

import tomllib

from cache_contract_data import WORKFLOW_DIR
from makefile_recipes import makefile_target
from nextest_budgets import global_timeout
from nextest_durations import seconds
from timeout_budgets import NEXTEST_CONFIG
from workflow_loading import job_steps, load_workflow, named_step

#: The CI job that owns the gate, and the step that invokes it.
KANI_JOB = "kani-smoke"
GATE_STEP = "Mutation patch compile gate"
GATE_COMMAND = "make test-kani-mutations"

#: Steps that must precede the gate, each for its own reason: the Kani
#: frontend the patches compile through, the build tools its prerequisite
#: checks, and the test runner it invokes.
PRECEDING_STEPS = (
    "Run Kani harnesses",
    "Install the build standard",
    "Install cargo-nextest",
)


def test_the_kani_lane_runs_the_mutation_compile_gate() -> None:
    """Require `kani-smoke` to reach the mutation gate, after its dependencies.

    Presence alone would be satisfied by a step placed anywhere in the job, so
    position is asserted too: a gate hoisted above the Kani install or the
    nextest install is present and cannot run, which is the shape a
    step-reordering edit produces.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / "ci.yml"), KANI_JOB)
    gate = named_step(steps, GATE_STEP)
    assert str(gate.get("run", "")).strip() == GATE_COMMAND, (
        f"{GATE_STEP} must run {GATE_COMMAND!r}, got {gate.get('run')!r}"
    )
    gate_index = steps.index(gate)
    for name in PRECEDING_STEPS:
        assert steps.index(named_step(steps, name)) < gate_index, (
            f"{name} must precede {GATE_STEP}: the gate compiles through the "
            "Kani frontend using the build tools and nextest those steps install"
        )


def test_the_gate_target_denies_warnings_through_the_kani_frontend() -> None:
    """Require the target the step names to do what the wiring claims.

    The step and the Make target are two halves of one gate, and this is where
    they meet. A step invoking a target that had been quietly reduced to a
    no-op would otherwise read as wired: the step is present, the job is green,
    and nothing compiles.
    """
    prerequisites, recipe = makefile_target("test-kani-mutations")
    assert "check-build-tools" in prerequisites, (
        "test-kani-mutations must depend on check-build-tools, so the gate fails "
        f"loudly when the linker or pinned toolchain is missing: {prerequisites!r}"
    )
    for fragment in (
        "--test kani_mutation_evidence_tests",
        "--profile kani",
        "$(GATE_RUSTFLAGS)",
        "--all-features",
    ):
        assert fragment in recipe, (
            f"the gate target must pass {fragment!r}; got {recipe!r}"
        )
    commands = [line for line in recipe.splitlines() if "nextest run" in line]
    assert len(commands) == 2, (
        f"the cold prewarm and patch sweep need separate Nextest budgets: {commands!r}"
    )
    assert "prewarm::prewarm_kani_mutation_target" in commands[0], (
        "the cold Kani compile must warm the shared target first"
    )
    assert (
        "negative::compile_kani_tree_rejects_trees_that_do_not_compile" in commands[0]
    ), (
        "the negative check must share the prewarm's warm target, so the sweep's "
        "'every patch compiles' result cannot be vacuous"
    )
    assert (
        "compile_guard::every_patched_tree_compiles_under_denied_warnings"
        in commands[1]
    ), "the timed patch sweep must run after the target is warm"


def test_the_kani_profile_covers_the_cold_compile_budget() -> None:
    """Keep the Kani test and whole-run limits in order.

    The job's own ``timeout-minutes`` ceiling is a different contract, held
    by ``runner_shape_test`` so that one place owns job timeouts.
    """
    config_text = NEXTEST_CONFIG.read_text(encoding="utf-8")
    config = tomllib.loads(config_text)
    profile = config["profile"]["kani"]
    whole_run = global_timeout(config_text, profile="kani")
    assert whole_run == 42 * 60, f"the Kani run budget changed: {whole_run}"

    # Select the sweep's own override rather than the first declared one. The
    # profile carries several by design — the prewarm, the negative control and
    # the sweep each widen one allowance — so positional unpacking would compare
    # the whole-run cap against whichever happened to be written first.
    sweep = next(
        (
            override
            for override in profile["overrides"]
            if "compile_guard::every_patched_tree_compiles_under_denied_warnings"
            in str(override.get("filter", ""))
        ),
        None,
    )
    assert sweep is not None, (
        "the patch sweep must carry its own allowance under the Kani profile: "
        f"{profile['overrides']!r}"
    )
    # Read the period through the shared duration reader; stripping a trailing
    # "s" by hand turns a future "1m" into a bare ValueError that names no
    # contract.
    period_seconds = seconds(sweep["slow-timeout"]["period"])
    test_budget = period_seconds * sweep["slow-timeout"]["terminate-after"]
    assert test_budget == 40 * 60, f"the mutation test allowance changed: {test_budget}"
    assert whole_run > test_budget, (
        "the Kani whole-run budget must exceed the test allowance"
    )
