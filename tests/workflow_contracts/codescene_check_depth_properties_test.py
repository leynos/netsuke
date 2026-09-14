"""Drive the checkout-depth rule over job shapes nobody wrote down.

The contract in ``codescene_check_depth_test`` holds this repository's
one gate job to the rule. One job cannot separate the rule from several
weaker ones: a reading that ignored order, or that accepted any
declared depth, agrees with it against a single correctly written job.
These generate the shapes instead, so the rule is stated over the whole
space of step lists rather than the one the repository happens to have.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from codescene_check_depth_invariants import (
    CHECKOUT_ACTION,
    CODESCENE_COVERAGE_ACTION,
    DEFAULT_FETCH_DEPTH,
    FULL_HISTORY_DEPTH,
    declared_depth,
    gate_is_prepared,
    runs_the_gate,
)
from hypothesis import given
from hypothesis import strategies as st

#: A step that is neither a checkout nor the gate. Present so a
#: generated job can separate the two without a rule reading adjacency
#: instead of order.
FILLER_STEP: typ.Final[dict[str, object]] = {"name": "unrelated", "run": "true"}

#: Depths that do not reach a merge base, as a workflow might write
#: them. The quoted forms reach `actions/checkout` as the same value as
#: the bare ones, so a reading that saw only integers would accept them.
insufficient_depths = st.sampled_from([1, 2, 5, 50, "1", "2", " 3 "])

#: Every spelling of the whole history. `0` is the documented value; the
#: quoted form is the same instruction.
full_history_depths = st.sampled_from([0, "0"])


def _checkout(depth: object = None) -> dict[str, object]:
    """Return a checkout step, optionally declaring a depth."""
    step: dict[str, object] = {"name": "checkout", "uses": f"{CHECKOUT_ACTION}@v7"}
    if depth is not None:
        step["with"] = {"fetch-depth": depth}
    return step


def _gate() -> dict[str, object]:
    """Return a step running the gate."""
    return {
        "name": "gate",
        "uses": f"{CODESCENE_COVERAGE_ACTION}@abc123",
        "with": {"mode": "check"},
    }


@given(depth=full_history_depths, before=st.lists(st.just(FILLER_STEP), max_size=3))
def test_a_full_history_checkout_before_the_gate_is_accepted(
    depth: object, before: list[dict[str, object]]
) -> None:
    """The rule must accept the shape the repository is meant to have.

    Unrelated steps may sit anywhere around the two that matter, so the
    rule is about order between them rather than adjacency.
    """
    steps = [*before, _checkout(depth), *before, _gate(), *before]

    assert gate_is_prepared(steps), (
        f"a fetch-depth {depth!r} checkout before the gate must be accepted; "
        f"got {steps}"
    )


@given(depth=insufficient_depths)
def test_a_shallow_checkout_before_the_gate_is_refused(depth: object) -> None:
    """Any depth short of the whole history leaves the merge base out.

    The action documents `mode: check` as needing `fetch-depth: 0`, so a
    rule accepting the deepest shallow value it happened to see would
    pass a job the CLI still cannot diff.
    """
    steps = [_checkout(depth), _gate()]

    assert not gate_is_prepared(steps), (
        f"fetch-depth {depth!r} is not the whole history and must be refused"
    )


@given(depth=full_history_depths, between=st.lists(st.just(FILLER_STEP), max_size=3))
def test_a_full_history_checkout_after_the_gate_is_refused(
    depth: object, between: list[dict[str, object]]
) -> None:
    """A checkout after the gate arrives too late to be read.

    The CLI runs when its step runs, so the history it sees is whatever
    exists then. A rule looking anywhere in the job accepts this and the
    job fails exactly as it did before.
    """
    steps = [_gate(), *between, _checkout(depth)]

    assert not gate_is_prepared(steps), (
        f"a checkout after the gate must be refused; got {steps}"
    )


@given(before=st.lists(st.just(FILLER_STEP), max_size=4))
def test_a_gate_with_no_checkout_is_refused(before: list[dict[str, object]]) -> None:
    """No checkout is the same empty history as a late one."""
    steps = [*before, _gate(), *before]

    assert not gate_is_prepared(steps), (
        f"a gate with no checkout must be refused; got {steps}"
    )


@given(depth=st.one_of(full_history_depths, insufficient_depths))
def test_a_job_without_the_gate_is_not_a_gate_job(depth: object) -> None:
    """Only `mode: check` reads history, so only it is held to the depth.

    An upload-only step submits the report and diffs nothing. A rule
    keyed on the action rather than the mode would demand a full history
    of jobs that never ask for one.
    """
    upload: dict[str, object] = {
        "name": "upload",
        "uses": f"{CODESCENE_COVERAGE_ACTION}@abc123",
        "with": {"mode": "upload"},
    }
    steps = [_checkout(depth), upload]

    assert not runs_the_gate(steps), (
        f"an upload-only step must not read as the gate; got {steps}"
    )


@given(
    value=st.one_of(
        st.none(),
        st.booleans(),
        st.text(max_size=8).filter(lambda text: not text.strip().isdigit()),
        st.lists(st.integers(), max_size=2),
    )
)
def test_a_depth_that_is_not_a_number_reads_as_the_default(value: object) -> None:
    """An unreadable depth must fail closed, not pass unexamined.

    `True` is the trap: it is an `int` in Python, so a reading that
    tested `isinstance(value, int)` first would take `fetch-depth: true`
    as depth one in one place and as something else in another.
    """
    step: dict[str, object] = {
        "name": "checkout",
        "uses": f"{CHECKOUT_ACTION}@v7",
        "with": {"fetch-depth": value},
    }

    assert declared_depth(step) == DEFAULT_FETCH_DEPTH, (
        f"fetch-depth {value!r} is not a depth and must read as the action's "
        f"default of {DEFAULT_FETCH_DEPTH}, not as {FULL_HISTORY_DEPTH}"
    )
