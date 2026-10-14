"""Hold the Kani lane's mutation compile gate to the workflow that runs it.

`make test-kani-mutations` compiles each mutation patch's patched tree through
the Kani frontend under denied warnings. The Make target is not enough on its
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

from cache_contract_data import WORKFLOW_DIR
from makefile_recipes import makefile_target
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
        "$(GATE_RUSTFLAGS)",
        "--all-features",
    ):
        assert fragment in recipe, (
            f"the gate target must pass {fragment!r}; got {recipe!r}"
        )
