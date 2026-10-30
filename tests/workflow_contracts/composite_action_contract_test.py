"""Apply the run-block expression rule to every composite action manifest.

`release_run_interpolation_test` holds the release workflows to the rule that
``${{ }}`` never reaches a ``run`` block as source text. A composite action is
a step body moved into its own file, and moving it does not move it out of
harm's way: until this module existed, the extraction the release path performs
took those bodies out of the workflow contract's reach entirely.

The rule is applied to every manifest under ``.github/actions``, not to a list
of the actions this branch adds. An allowlist is the weakening the mission
forbids, and a path-scoped rule would leave the next action uncovered until
somebody remembered to extend it. The measured state is green today — none of
the existing actions carries an expression inside a ``run`` block — so the
sweep constrains an invariant the tree already satisfies rather than demanding
new work.

Two further checks keep that sweep honest. One counts the manifests found, so a
sweep that reaches nothing cannot pass by having nothing to check; the other
injects an expression into every run block in turn, so a predicate that never
fires is caught by an example rather than trusted because the tree is green.

Run via ``make test-workflow-contracts``.
"""

import copy

import pytest
from action_run_sources import (
    EXPRESSION,
    action_manifest_paths,
    action_run_violations,
    action_sources,
    action_steps,
)

#: Appended to a step's body to make it unsafe. Names a real expression so the
#: diagnostic reads as a plausible mistake rather than a bare token.
INJECTED_EXPRESSION = "\necho '${{ github.ref_name }}'"


def test_every_composite_action_keeps_expressions_out_of_run_blocks() -> None:
    """Reject any expression arriving in a composite action's shell source.

    Every manifest is checked, so a value the caller passes through ``env:``
    stays a substituted value while one written into the body would be executed
    by the runner's shell.
    """
    violations = [
        violation
        for action_name, document in action_sources().items()
        for violation in action_run_violations(action_name, document)
    ]

    assert not violations, (
        "composite action run blocks must not interpolate expressions; pass the "
        f"value through the step environment instead: {violations}"
    )


def test_the_sweep_reaches_every_manifest_the_tree_declares() -> None:
    """Require the sweep to find the manifests it claims to have checked.

    A walk that matched nothing would make the contract above pass without
    inspecting a single action. The count is compared against an independent
    listing rather than against a hard-coded number, so adding an action
    extends the sweep instead of failing this check.
    """
    manifest_paths = action_manifest_paths()
    sources = action_sources()

    assert manifest_paths, (
        "the sweep found no composite action manifests, so it inspected nothing"
    )
    assert len(sources) == len(manifest_paths), (
        "every manifest path must load into exactly one parsed document"
    )


def test_the_rule_rejects_an_expression_injected_into_each_run_block() -> None:
    """Inject one expression per run block and require that step alone to fail.

    This is the negative mutation the blanket sweep needs. Without it, a
    predicate that never fired would still leave the sweep green, and the
    contract would be indistinguishable from one that checks nothing.
    """
    checked = 0
    for action_name, baseline in action_sources().items():
        for index, step in enumerate(action_steps(action_name, baseline)):
            run = step.get("run")
            if not isinstance(run, str):
                continue
            document = copy.deepcopy(baseline)
            mutated = action_steps(action_name, document)[index]
            mutated["run"] = run + INJECTED_EXPRESSION
            step_name = str(mutated.get("name", "<unnamed>"))

            expected = (
                f"{action_name} step {index} ({step_name}): "
                f"run block contains '{EXPRESSION}'"
            )
            assert action_run_violations(action_name, document) == [expected], (
                "the injected expression must be the sole violation in that step"
            )
            checked += 1

    assert checked, "at least one composite action must declare a run block"


def test_every_step_reports_a_parseable_shape() -> None:
    """Require each declared step to be a mapping the contracts can read.

    ``action_steps`` fails on any other shape, but only for the actions a test
    happens to reach. Walking them all here means a malformed manifest is
    reported by the contract that owns the shape rather than by whichever
    unrelated test read it first.
    """
    for action_name, document in action_sources().items():
        for index, step in enumerate(action_steps(action_name, document)):
            assert isinstance(step, dict), (
                f"{action_name} step {index} must be a mapping"
            )
            if "run" in step:
                assert step.get("shell"), (
                    f"{action_name} step {index} must declare a shell for its run"
                )


def test_a_manifest_without_a_runs_mapping_is_refused() -> None:
    """Fail on a manifest whose ``runs`` section is absent or not a mapping.

    The sweep's fixtures are the real tree, so this exercises the failure path
    with a synthetic document instead: a manifest that declares no ``runs``
    must be refused rather than silently contributing no steps to the sweep.
    """
    with pytest.raises(pytest.fail.Exception, match="runs"):
        action_steps("demo/action.yml", {})

    with pytest.raises(pytest.fail.Exception, match="runs"):
        action_steps("demo/action.yml", {"runs": []})
