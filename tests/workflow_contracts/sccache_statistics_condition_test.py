"""Evaluate each statistics step's condition across the fallback paths.

`setup_rust_sccache_test.py` holds the exact `if:` string. A string match shows
the text is as written and nothing about what a runner does with it, so this
module evaluates the condition the way GitHub Actions would for each case that
matters: `setup-rust` reporting a fallback, a started server, and no server
status at all, each with the job still green and with an earlier step failed.

A fallback has to skip the report in both job states, because an uncached job
has no statistics to publish. Every other status has to run it, and a failed
build has to run it too, because a failed build is when the numbers are wanted.

The evaluator mirrors GitHub's implicit `success() &&` on a condition with no
status function. It supports only `always()`, `success()`, `failure()`, an output
compared with `==` or `!=` against a string literal, `&&` and `||`. Anything
else raises, so a condition it cannot evaluate fails the contract instead of
being guessed at.

The last test closes the other half of the step: when the condition does let it
run, the step must hand its work to the checked-in report script with the
backend `setup-rust` selected. The script's own behaviour is covered by
`scripts/tests/test_ci_report_sccache_stats.py`, so this module never executes
a step's `run` as shell.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

import pytest
from cache_contract_data import SETUP_RUST_SCCACHE_JOBS, WORKFLOW_DIR
from sccache_compile_step_data import SCCACHE_REPORT_SCRIPT
from workflow_loading import job_steps, load_workflow, named_step

SETUP_RUST_ID = "setup-rust"
_COMPARISON = re.compile(
    r"^steps\.(?P<step>[\w-]+)\.outputs\.(?P<name>[\w-]+)"
    r"(?P<op>==|!=)'(?P<literal>[^']*)'$"
)

#: `(setup-rust's sccache-status, whether an earlier step failed, runs?)`.
CASES: typ.Final = [
    pytest.param(("fallback", False, False), id="fallback-green-skips"),
    pytest.param(("fallback", True, False), id="fallback-after-a-failure-skips"),
    pytest.param(("started", False, True), id="started-green-runs"),
    pytest.param(("started", True, True), id="started-after-a-failure-runs"),
    pytest.param(("", False, True), id="no-status-green-runs"),
    pytest.param(("", True, True), id="no-status-after-a-failure-runs"),
]


def _with_implicit_success(condition: str) -> str:
    """Return the condition text GitHub would evaluate, braces stripped.

    GitHub prepends `success() &&` to a condition that names no status
    function, so a bare comparison does not run after a failed step.

    Returns
    -------
    str
        The condition with an implicit `success() &&` made explicit.
    """
    text = condition.strip().removeprefix("${{").removesuffix("}}").strip()
    if re.search(r"\b(?:always|success|failure|cancelled)\(\)", text):
        return text
    return f"success() && {text}" if text else "success()"


def _unreadable_terms(text: str) -> list[str]:
    """Return the terms of `text` that are outside the supported grammar."""
    functions = ("always()", "success()", "failure()")
    terms = [term.strip() for term in re.split(r"&&|\|\|", text)]
    return [
        term
        for term in terms
        if term not in functions and _COMPARISON.match(re.sub(r"\s+", "", term)) is None
    ]


def _arm_holds(
    arm: str, outputs: dict[tuple[str, str], str], *, job_failed: bool
) -> bool:
    """Return whether every `&&` conjunct of one `||` arm holds."""
    return all(
        _atom(atom.strip(), outputs, job_failed=job_failed) for atom in arm.split("&&")
    )


def evaluate(
    condition: str, outputs: dict[tuple[str, str], str], *, job_failed: bool
) -> bool:
    """Evaluate a step `if:` the way GitHub Actions would for the given state.

    Parameters
    ----------
    condition : str
        The step's `if:` text. An empty string means no condition, and a
        condition that names no status function gets an implicit `success() &&`.
        The grammar is `&&` and `||` over `always()`, `success()`, `failure()`
        and `steps.<id>.outputs.<name>` compared with `==` or `!=` against a
        single-quoted string literal. `||` binds more loosely than `&&`.
    outputs : dict[tuple[str, str], str]
        Step outputs by `(step id, output name)`. A missing output reads as an
        empty string, as it does on a runner.
    job_failed : bool
        Whether an earlier step failed, which decides `success()` and
        `failure()`.

    Returns
    -------
    bool
        Whether the step would run.

    Raises
    ------
    ValueError
        If any term is outside the grammar above. An expression the evaluator
        cannot read must stop the contract, not be guessed at.

    Examples
    --------
    >>> evaluate("always() && steps.s.outputs.x != 'fallback'", {("s", "x"): "ok"},
    ...          job_failed=True)
    True
    """
    text = _with_implicit_success(condition)
    unreadable = _unreadable_terms(text)
    if unreadable:
        message = f"cannot evaluate {unreadable!r}"
        raise ValueError(message)
    return any(
        _arm_holds(arm, outputs, job_failed=job_failed) for arm in text.split("||")
    )


def _atom(atom: str, outputs: dict[tuple[str, str], str], *, job_failed: bool) -> bool:
    """Evaluate one comparison or status function."""
    functions = {
        "always()": True,
        "success()": not job_failed,
        "failure()": job_failed,
    }
    if atom in functions:
        return functions[atom]
    match = _COMPARISON.match(re.sub(r"\s+", "", atom))
    assert match is not None, f"{atom!r} must have been validated by evaluate"
    actual = outputs.get((match["step"], match["name"]), "")
    equal = actual == match["literal"]
    return equal if match["op"] == "==" else not equal


@pytest.mark.parametrize(("workflow_name", "job_name"), list(SETUP_RUST_SCCACHE_JOBS))
@pytest.mark.parametrize("case", CASES)
def test_the_statistics_step_runs_exactly_when_there_are_statistics(
    workflow_name: str, job_name: str, case: tuple[str, bool, bool]
) -> None:
    """A fallback skips the report in both job states; everything else runs it."""
    status, job_failed, runs = case
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    show = named_step(steps, "Show sccache statistics")
    outputs = {(SETUP_RUST_ID, "sccache-status"): status}
    assert evaluate(str(show.get("if", "")), outputs, job_failed=job_failed) is runs, (
        f"{workflow_name} {job_name}: with sccache-status={status!r} and "
        f"job_failed={job_failed}, 'Show sccache statistics' must "
        f"{'run' if runs else 'be skipped'} (condition {show.get('if')!r})"
    )


@pytest.mark.parametrize(
    ("condition", "status", "job_failed", "expected"),
    [
        (
            "always() && steps.setup-rust.outputs.sccache-status != 'fallback'",
            "fallback",
            True,
            False,
        ),
        (
            "always() || steps.setup-rust.outputs.sccache-status != 'fallback'",
            "fallback",
            False,
            True,
        ),
        (
            "steps.setup-rust.outputs.sccache-status != 'fallback'",
            "started",
            True,
            False,
        ),
        (
            "success() && steps.setup-rust.outputs.sccache-status != 'fallback'",
            "started",
            True,
            False,
        ),
        (
            "always() && steps.setup-rust.outputs.sccache-status == 'fallback'",
            "fallback",
            False,
            True,
        ),
        ("always()", "fallback", False, True),
        ("", "started", True, False),
    ],
    ids=[
        "the-lane-condition-skips-a-fallback-after-a-failure",
        "an-or-composed-guard-runs-on-a-fallback",
        "a-bare-guard-gets-an-implicit-success-and-loses-a-failed-build",
        "success-only-loses-a-failed-build",
        "an-inverted-guard-runs-on-a-fallback",
        "an-omitted-guard-runs-on-a-fallback",
        "no-condition-means-success-only",
    ],
)
def test_the_evaluator_tells_a_working_guard_from_a_broken_one(
    condition: str, status: str, *, job_failed: bool, expected: bool
) -> None:
    """The narrow half: each way of breaking the guard changes the outcome."""
    outputs = {(SETUP_RUST_ID, "sccache-status"): status}
    assert evaluate(condition, outputs, job_failed=job_failed) is expected, (
        f"{condition!r} with sccache-status={status!r}, job_failed={job_failed} "
        f"must evaluate to {expected}"
    )


def test_an_expression_the_evaluator_cannot_read_is_refused() -> None:
    """Guessing at unsupported syntax would pass a condition nobody evaluated."""
    with pytest.raises(ValueError, match="cannot evaluate"):
        evaluate("!(always())", {}, job_failed=False)


@pytest.mark.parametrize(("workflow_name", "job_name"), list(SETUP_RUST_SCCACHE_JOBS))
def test_a_started_job_hands_the_backend_to_the_report_script(
    workflow_name: str, job_name: str
) -> None:
    """The step runs the checked-in script and names the backend it selected.

    A fallback never reaches this script, because the condition above skips the
    step. What the script does when it does run — the text and JSON files, the
    summary section, and the backend line — belongs to the script's own suite,
    `scripts/tests/test_ci_report_sccache_stats.py`, which drives it with a
    stand-in `sccache`. Running that behaviour from here would mean executing
    the step's `run` as shell, which is exactly the inline shell this lane no
    longer carries.
    """
    steps = job_steps(load_workflow(WORKFLOW_DIR / workflow_name), job_name)
    show = named_step(steps, "Show sccache statistics")
    assert str(show.get("run", "")).strip() == SCCACHE_REPORT_SCRIPT, (
        f"{workflow_name} {job_name} must report through "
        f"{SCCACHE_REPORT_SCRIPT!r}, got {show.get('run')!r}"
    )
    env = show.get("env")
    assert isinstance(env, dict), (
        f"{workflow_name} {job_name} must pass the selected backend to the "
        "script; the statistics' own `Cache location` cannot stand in for it"
    )
    assert env.get("SCCACHE_BACKEND") == (
        f"${{{{ steps.{SETUP_RUST_ID}.outputs.cache-backend }}}}"
    ), (
        f"{workflow_name} {job_name} must set SCCACHE_BACKEND from "
        f"steps.{SETUP_RUST_ID}.outputs.cache-backend, got "
        f"{env.get('SCCACHE_BACKEND')!r}"
    )
