"""Hold the finalization action's own steps to their contract.

The action is where a finalization run actually touches a release, so this
module states what its steps may do: which environment each one receives,
which of them may hold the token, how long each adapter invocation may run,
what the token-bearing steps may resolve and build, and that the single
mutating command is the contracted one. The test module
keeps the job that calls the action and the mutation cases that prove these
assertions fire; the assertions themselves live here so each file stays a size
a reviewer can read in one sitting.

Run via ``make test-workflow-contracts``.
"""

import re

from release_finalize_contract_support import (
    EXPECTED_ENVIRONMENTS,
    EXPECTED_INPUTS,
    EXPECTED_OUTPUTS,
    FINALIZE_COMMAND,
    FINALIZE_STEP,
    FINALIZE_STEP_TIMEOUT_MINUTES,
    FORBIDDEN_IN_RUN,
    MAX_COMMAND_BOUND_SECONDS,
    PLAN_PRESENCE_GUARD,
    PLAN_RESET,
    PLAN_STEP,
    RESOLVE_STEP,
    SNAPSHOT_STEP,
    STEP_NAMES,
    TOKEN_STEPS,
    load_action_steps,
    run_tokens,
)
from workflow_loading import REPO_ROOT, named_step, require_mapping

#: ``timeout --kill-after=30s 300s`` and the line continuation that keeps the
#: bounded command on the next line, where :func:`run_tokens` finds it.
_COMMAND_BOUND = re.compile(r"timeout --kill-after=\d+s (\d+)s \\")

#: The steps whose ``run`` script invokes an adapter, and so needs a bound. The
#: validate step runs only shell builtins and `mkdir`.
BOUNDED_STEPS = (RESOLVE_STEP, SNAPSHOT_STEP, PLAN_STEP, FINALIZE_STEP)

#: A whole `inputs['github-token'] != ''` comparison, which yields a boolean
#: rather than the token. Removing it leaves any genuine reference in place.
_PRESENCE_TEST = re.compile(
    r"inputs(?:\[['\"]github-token['\"]\]|\.github-token)\s*[!=]=\s*''"
)

#: The adapter the two token-bearing steps run. It is the only script here that
#: resolves its own dependencies at run time, with a write token in scope.
FINALIZER_SCRIPT = "scripts/finalize_github_release.py"

#: The inline-metadata table a PEP 723 block needs for any setting beyond
#: `dependencies` and `requires-python`.
_UV_TABLE = "# [tool.uv]"

#: The resolution cutoff itself. Dates only, so a malformed value cannot pass
#: as a cutoff the resolver would reject anyway.
_EXCLUDE_NEWER = re.compile(r'^# exclude-newer = "\d{4}-\d{2}-\d{2}T', re.MULTILINE)

#: The flag that keeps `uv` from building a source distribution. The cutoff
#: bounds which releases may be chosen; it says nothing about what a chosen
#: package's build backend does, and a build runs arbitrary code in the same
#: environment as the step's write token.
NO_BUILD_FLAG = "--no-build"


def _receives_token_value(value: object) -> bool:
    """Report whether an environment value resolves to the token itself.

    A comparison against the empty string asks only whether the caller supplied
    one, which is all a validation step needs; any other mention of the input
    hands the credential to a step that has no reason to hold it.

    Returns
    -------
    bool
        ``True`` when the value still names the token input once its presence
        tests are removed.
    """
    return "github-token" in _PRESENCE_TEST.sub("", str(value))


def _assert_step_shape(step: dict[str, object]) -> int | None:
    """Require strict shell handling, environment-only substitutions, and bounds.

    Returns
    -------
    int | None
        The step's command bound in seconds, or ``None`` for a step that runs
        no adapter and so needs none.
    """
    name = str(step.get("name"))
    _assert_shell_shape(name, step)
    return _assert_step_bounds(name, step)


def _assert_shell_shape(name: str, step: dict[str, object]) -> None:
    """Require the shell and substitution discipline every step must keep."""
    assert step.get("shell") == "bash", f"{name} must run under Bash"
    run = step.get("run")
    assert isinstance(run, str), f"{name} must be a shell script"
    assert run.splitlines()[0] == "set -euo pipefail", (
        f"{name} must enable strict shell handling"
    )
    assert "${{" not in run, (
        f"{name} must receive expressions through env, where they are "
        "evaluated once rather than by the shell"
    )
    for forbidden in FORBIDDEN_IN_RUN:
        assert forbidden not in run, (
            f"{name} must not call {forbidden!r}; the adapter owns that boundary"
        )


def _assert_step_bounds(name: str, step: dict[str, object]) -> int | None:
    """Require the bound to live where the runner actually honours it.

    Returns
    -------
    int | None
        The adapter command's bound in seconds, or ``None`` when the step runs
        no adapter.
    """
    # The runner ignores this key inside a composite action, so a step that
    # carried one would look bounded and be nothing of the sort.
    assert "timeout-minutes" not in step, (
        f"{name} must not carry timeout-minutes: the runner does not support it "
        "in a composite action (actions/runner#1979)"
    )
    if name in BOUNDED_STEPS:
        return _assert_command_bound(name, str(step.get("run")))
    return None


def _assert_command_bound(name: str, run: str) -> int:
    """Require one adapter invocation to run under a shell-level bound.

    Returns
    -------
    int
        The bound in seconds, so the caller can weigh the commands together
        against the ceiling that has to contain their sum.
    """
    lines = run.splitlines()
    start = next(index for index, line in enumerate(lines) if "uv run " in line)
    assert start > 0, f"{name} must invoke the adapter on its own command line"
    bound = _COMMAND_BOUND.fullmatch(lines[start - 1].strip())
    assert bound, f"{name} must bound its adapter invocation with `timeout`"
    seconds = int(bound.group(1))
    assert 0 < seconds <= MAX_COMMAND_BOUND_SECONDS, (
        f"{name} must bound its adapter invocation within "
        f"{MAX_COMMAND_BOUND_SECONDS} seconds, got {bound.group(1)}"
    )
    return seconds


def _assert_interface(action: dict[str, object]) -> None:
    """Check the action's contracted inputs and outputs."""
    inputs = require_mapping(action.get("inputs"), "action inputs")
    assert set(inputs) == set(EXPECTED_INPUTS), (
        "the action must expose exactly the contracted inputs"
    )
    for name, required in EXPECTED_INPUTS.items():
        spec = require_mapping(inputs[name], f"{name} input")
        assert spec.get("required") is required, f"{name} required flag changed"

    outputs = require_mapping(action.get("outputs"), "action outputs")
    assert set(outputs) == set(EXPECTED_OUTPUTS), (
        "the action must expose exactly the contracted outputs"
    )
    for name, value in EXPECTED_OUTPUTS.items():
        output = require_mapping(outputs[name], f"{name} output")
        assert output.get("value") == value, f"{name} output mapping changed"


def _assert_step_environments(steps: list[dict[str, object]]) -> list[int]:
    """Check each step's shell, credential routing, and bound.

    Returns
    -------
    list[int]
        The bound each adapter invocation runs under, in seconds, so the caller
        can weigh them together against the step that contains them.
    """
    bounds: list[int] = []
    for step in steps:
        name = str(step.get("name"))
        bound = _assert_step_shape(step)
        if bound is not None:
            bounds.append(bound)
        environment = require_mapping(step.get("env"), f"{name} environment")
        # The token rule is asserted before the environment comparison, and
        # independently of it: the expected-environment map is what recorded
        # the leak as correct in the first place — the validate step held the
        # credential only to test that it was non-empty — so a rule that only
        # restated that map could not catch the same mistake twice.
        if name not in TOKEN_STEPS:
            leaked = sorted(
                variable
                for variable, value in environment.items()
                if _receives_token_value(value)
            )
            assert not leaked, (
                f"{name} must not forward the token expression under any name; "
                f"only {TOKEN_STEPS} reach GitHub, got {leaked}"
            )
        else:
            # Holding the token is what makes this mandatory: resolution and
            # installation run in the same environment, so a source build would
            # execute a build backend beside the credential. The flag is read
            # from the invocation's tokens rather than from the script's text,
            # because the step's own comments name the flag.
            assert NO_BUILD_FLAG in run_tokens(str(step.get("run"))), (
                f"{name} holds the token, so its `uv run` must carry "
                f"{NO_BUILD_FLAG!r}; without it a dependency with no wheel "
                "runs its build backend with the credential in scope"
            )
        assert environment == EXPECTED_ENVIRONMENTS[name], (
            f"{name} must forward exactly its contracted environment names"
        )
        assert ("GH_TOKEN" in environment) is (name in TOKEN_STEPS), (
            f"{name} must hold a token only when it must reach GitHub"
        )
    return bounds


def _assert_bounds_fit_the_step(bounds: list[int]) -> None:
    """Require the bounds in total to fit inside the step that contains them."""
    # Each bound is only sound if the bounds together still fit: four commands
    # may each sit at their individual cap and yet outlast the ceiling, which
    # would report the hang against the job rather than against the command
    # that stopped answering.
    total = sum(bounds)
    ceiling = FINALIZE_STEP_TIMEOUT_MINUTES * 60
    assert total < ceiling, (
        f"the adapter bounds total {total}s, which must stay under the "
        f"{ceiling}s step ceiling so a hung command is reported against the step"
    )


def _assert_plan_step(steps: list[dict[str, object]]) -> None:
    """Require the plan step to refuse a run that recorded no plan."""
    run = str(named_step(steps, PLAN_STEP).get("run"))
    assert PLAN_RESET in run, (
        "the plan step must delete any earlier plan before planning: a stale "
        "file would let an unreadable plan stand in for this run's answer"
    )
    assert PLAN_PRESENCE_GUARD in run, (
        "the plan step must refuse a planner status that wrote no plan; usage "
        "errors and resolution failures exit with the statuses it accepts"
    )


def _assert_finalize_step(steps: list[dict[str, object]]) -> None:
    """Require the finalize step to run the contracted command and fail on it."""
    finalize = named_step(steps, FINALIZE_STEP)
    run = str(finalize.get("run"))
    assert run_tokens(run) == FINALIZE_COMMAND, (
        "the finalize step must run exactly the contracted adapter command"
    )
    # Leaving the release a draft on a refusal is implemented by not
    # swallowing the adapter's exit status: any of these forms would turn a
    # refusal into a green step that published nothing.
    assert "continue-on-error" not in finalize, (
        "the finalize step must not continue past a refused publication"
    )
    assert "set +e" not in run, (
        "the finalize step must let the adapter's exit status fail the step"
    )
    assert "|| " not in run, "the finalize step must not fall back after a refusal"


def assert_adapter_pins_its_closure(source: str | None = None) -> None:
    """Require the token-bearing adapter to constrain its resolution.

    Parameters
    ----------
    source
        The adapter's text. The contract reads the checked-in script, and the
        control passes a block whose cutoff was removed, so the assertion is
        shown to notice the change rather than to pass on any text.
    """
    text = (
        source
        if source is not None
        else (REPO_ROOT / FINALIZER_SCRIPT).read_text(encoding="utf-8")
    )
    assert _UV_TABLE in text, (
        f"{FINALIZER_SCRIPT} must carry a [tool.uv] table: the PEP 723 block "
        "has no other place for a resolution setting"
    )
    assert _EXCLUDE_NEWER.search(text), (
        f"{FINALIZER_SCRIPT} must set `exclude-newer`: its two direct pins "
        "leave the transitive closure to resolve afresh in a step that holds a "
        "write token, and the cutoff is what refuses a later dependency release"
    )


def assert_action_contract(action: dict[str, object]) -> None:
    """Check the action's interface, token routing, and single mutating call."""
    _assert_interface(action)
    steps = load_action_steps(action)
    assert [step.get("name") for step in steps] == list(STEP_NAMES), (
        "the action's steps must keep their contracted order"
    )
    _assert_bounds_fit_the_step(_assert_step_environments(steps))
    _assert_plan_step(steps)
    _assert_finalize_step(steps)
    assert_adapter_pins_its_closure()
