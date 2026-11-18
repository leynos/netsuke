"""Property-test matrix selector resolution against a direct model.

`runner_matrix` exists so the self-hosted label inventory cannot fail open: a
selector over a job's own matrix must contribute exactly the labels that axis
declares, and a selector the job cannot resolve must come back as itself. The
example-based suite beside this module pins the documented shapes; these
properties hold both guarantees over generated matrices, where an empty
declaration, an axis sharing values with another axis, or an ``include`` entry
for a different axis is easy to miss.

Run via ``make test-workflow-contracts``.
"""

import operator

from hypothesis import given, settings
from hypothesis import strategies as st
from runner_matrix import selected_labels

_AXIS_INITIAL = st.sampled_from("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_")
_AXIS_REST = st.text(
    alphabet="abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-",
    max_size=8,
)
#: Names the matrix grammar accepts, excluding the reserved ``include`` and
#: ``strategy`` keys a matrix uses for its own structure.
_AXIS = st.builds(operator.add, _AXIS_INITIAL, _AXIS_REST).filter(
    lambda name: name not in {"include", "strategy"}
)
_LABEL = st.text(
    alphabet="abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-",
    min_size=1,
    max_size=16,
)


def _selector(axis: str) -> str:
    """Return the whole matrix expression that reads one axis."""
    return "${{ matrix." + axis + " }}"


@st.composite
def _resolvable_case(draw: st.DrawFn) -> tuple[str, dict[str, object], list[str]]:
    """Generate a job that declares an axis and the labels it must resolve to.

    The other axis and its values are generated too, so a resolver that read
    every ``include`` entry instead of the selected axis would return a
    different list rather than passing on a matrix that had only one axis.

    Returns
    -------
    tuple[str, dict[str, object], list[str]]
        The axis, the job that declares it, and the labels it must resolve to.
    """
    axis = draw(_AXIS)
    other_axis = draw(_AXIS.filter(lambda name: name != axis))
    declared = draw(st.lists(_LABEL, min_size=1, max_size=4))
    included = draw(st.lists(_LABEL, max_size=4))
    other_values = draw(st.lists(_LABEL, min_size=1, max_size=3))
    include = [{axis: value} for value in included]
    include += [{other_axis: value} for value in other_values]
    matrix: dict[str, object] = {axis: declared, "include": include}
    if draw(st.booleans()):
        matrix[other_axis] = other_values
    return axis, {"strategy": {"matrix": matrix}}, [*declared, *included]


@st.composite
def _unresolvable_case(draw: st.DrawFn) -> tuple[str, dict[str, object]]:
    """Generate a job that names no leg of the axis its selector reads.

    The matrix may be missing entirely, present but empty, or declare only
    another axis; all three leave the selector unread, and a resolver that
    dropped it would remove the label from the inventory.

    Returns
    -------
    tuple[str, dict[str, object]]
        The axis the selector reads and a job that does not declare it.
    """
    axis = draw(_AXIS)
    other_axis = draw(_AXIS.filter(lambda name: name != axis))
    values = draw(st.lists(_LABEL, min_size=1, max_size=4))
    include = [{other_axis: value} for value in values]
    matrix: dict[str, object] = {other_axis: values, "include": include}
    shape = draw(st.sampled_from(("no-strategy", "empty-matrix", "other-axis")))
    if shape == "no-strategy":
        return axis, {}
    if shape == "empty-matrix":
        return axis, {"strategy": {"matrix": {}}}
    return axis, {"strategy": {"matrix": matrix}}


@settings(max_examples=200, derandomize=True, deadline=None)
@given(case=_resolvable_case())
def test_a_resolvable_selector_returns_exactly_its_own_axis(
    case: tuple[str, dict[str, object], list[str]],
) -> None:
    """Every declared leg counts, and no other axis's legs do."""
    axis, job, expected = case
    selector = _selector(axis)
    actual = selected_labels(selector, job)
    assert actual == expected, (
        f"{selector} must resolve to {expected!r}, got {actual!r}"
    )


@settings(max_examples=200, derandomize=True, deadline=None)
@given(case=_unresolvable_case())
def test_an_unresolvable_selector_is_returned_as_itself(
    case: tuple[str, dict[str, object]],
) -> None:
    """A selector the job cannot read stays visible, never empty."""
    axis, job = case
    selector = _selector(axis)
    actual = selected_labels(selector, job)
    assert actual == [selector], (
        f"{selector} is unreadable in {job!r}, so it must be returned as itself, "
        f"got {actual!r}"
    )


@settings(max_examples=200, derandomize=True, deadline=None)
@given(case=_resolvable_case(), value=_LABEL)
def test_a_bare_label_is_never_resolved_through_the_matrix(
    case: tuple[str, dict[str, object], list[str]], value: str
) -> None:
    """A literal names itself even when the job's matrix holds the same text."""
    _, job, _ = case
    actual = selected_labels(value, job)
    assert actual == [value], (
        f"the bare label {value!r} must pass through unchanged, got {actual!r}"
    )
