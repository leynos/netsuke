"""Property-test matrix selector resolution against a direct model.

`runner_matrix` exists so the self-hosted label inventory cannot fail open: a
selector over a job's own matrix must contribute exactly the labels the legs
GitHub creates, and a selector the job cannot resolve must come back as
itself. The example-based suite beside this module pins the documented shapes;
these properties hold both guarantees over generated matrices, where an
excluded leg, an include row that re-adds an excluded value, a later row that
overrides an earlier one, or an axis sharing values with another axis is easy
to miss.

The selector's two leaf readers are property-tested here as well. A quoted
literal must read back as the text it was written from, however many quotes
that text holds, and an empty literal must select nothing at all. A matrix
selector must be read only from a whole expression that names one axis: text
that merely contains such an expression selects no label the resolver may
silently drop.

Run via ``make test-workflow-contracts``.
"""

import operator

from actions_expressions import matrix_axis, static_label
from hypothesis import given, settings
from hypothesis import strategies as st
from runner_matrix import selected_labels

_AXIS_INITIAL = st.sampled_from("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_")
_AXIS_REST = st.text(
    alphabet="abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-",
    max_size=8,
)
#: Names the matrix grammar accepts, excluding the case-insensitive reserved
#: keys a matrix uses for its own structure.
_AXIS = st.builds(operator.add, _AXIS_INITIAL, _AXIS_REST).filter(
    lambda name: name.lower() not in {"include", "exclude", "strategy"}
)
_LABEL = st.text(
    alphabet="abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-",
    min_size=1,
    max_size=16,
)
#: Text that survives ``strip()``, so it cannot be trimmed from around a value.
_VISIBLE_TEXT = st.text(min_size=1, max_size=8).filter(lambda text: bool(text.strip()))


def _selector(axis: str) -> str:
    """Return the whole matrix expression that reads one axis."""
    return "${{ matrix." + axis + " }}"


def _distinct(values: list[str]) -> list[str]:
    """Return values in first-occurrence order, without repeats."""
    return list(dict.fromkeys(values))


@st.composite
def _case_variant(draw: st.DrawFn, name: str) -> str:
    """Return ``name`` with each character independently upper- or lowercase."""
    return "".join(
        character.upper() if draw(st.booleans()) else character.lower()
        for character in name
    )


@st.composite
def _added_axis_values(draw: st.DrawFn, declared: list[str]) -> list[str]:
    """Generate include-row values for one axis: some declared, some new.

    Returns
    -------
    list[str]
        Declared values first, then distinct values the axis does not declare.
    """
    again = draw(st.lists(st.sampled_from(declared), max_size=3))
    fresh = draw(
        st.lists(
            _LABEL.filter(lambda label: label not in declared),
            max_size=3,
            unique=True,
        )
    )
    return [*again, *fresh]


def _expected_values(
    surviving: list[str], include: list[dict[str, str]], axis: str
) -> list[str]:
    """Return the axis values the include rows add beyond the survivors."""
    included = [row[axis] for row in include if axis in row]
    return _distinct([
        *surviving,
        *(value for value in included if value not in surviving),
    ])


@st.composite
def _axis_case(draw: st.DrawFn) -> tuple[str, dict[str, object], list[str]]:
    """Generate a matrix whose effective values for one axis are known.

    The matrix combines a declared axis, excluded declared values, include
    rows that re-add declared values, include rows that add distinct values,
    and include rows for another declared axis. The expected list is therefore
    the surviving declared values followed by the distinct include values the
    surviving legs do not already hold.

    Returns
    -------
    tuple[str, dict[str, object], list[str]]
        The selector, the job that declares the matrix, and the labels the
        selector must resolve to.
    """
    axis = draw(_AXIS)
    other_axis = draw(_AXIS.filter(lambda name: name.lower() != axis.lower()))
    declared = draw(st.lists(_LABEL, min_size=1, max_size=4, unique=True))
    excluded = draw(
        st.lists(st.sampled_from(declared), max_size=len(declared), unique=True)
    )
    other_values = draw(st.lists(_LABEL, min_size=1, max_size=3))
    include = [{axis: value} for value in draw(_added_axis_values(declared))]
    include += [{other_axis: value} for value in other_values]
    matrix: dict[str, object] = {
        axis: declared,
        other_axis: other_values,
        "exclude": [{axis: value} for value in excluded],
        "include": include,
    }
    surviving = [value for value in declared if value not in excluded]
    expected = _expected_values(surviving, include, axis)
    selector = _selector(draw(_case_variant(axis)))
    # With every leg excluded and no include row naming the axis, there are no
    # values at all. `selected_labels` keeps an unreadable selector visible as
    # itself in that case, so the model includes that documented fallback.
    return selector, {"strategy": {"matrix": matrix}}, expected or [selector]


@st.composite
def _include_only_case(draw: st.DrawFn) -> tuple[str, dict[str, object], list[str]]:
    """Generate an include-only matrix with one leg per row.

    Returns
    -------
    tuple[str, dict[str, object], list[str]]
        The selector, the job, and the labels the include rows declare.
    """
    axis = draw(_AXIS)
    other_axis = draw(_AXIS.filter(lambda name: name.lower() != axis.lower()))
    values = draw(st.lists(_LABEL, min_size=1, max_size=4, unique=True))
    other_values = draw(st.lists(_LABEL, min_size=1, max_size=3))
    include = [{axis: value} for value in values]
    include += [{other_axis: value} for value in other_values]
    job: dict[str, object] = {"strategy": {"matrix": {"include": include}}}
    return _selector(draw(_case_variant(axis))), job, values


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
    other_axis = draw(_AXIS.filter(lambda name: name.lower() != axis.lower()))
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
@given(case=_axis_case())
def test_a_resolvable_selector_returns_the_effective_axis_values(
    case: tuple[str, dict[str, object], list[str]],
) -> None:
    """Excluded legs drop out, and includes contribute only final values."""
    selector, job, expected = case
    actual = selected_labels(selector, job)
    assert actual == expected, (
        f"{selector} must resolve to {expected!r}, got {actual!r} from {job!r}"
    )


@settings(max_examples=200, derandomize=True, deadline=None)
@given(case=_include_only_case())
def test_an_include_only_matrix_keeps_every_row_as_its_own_leg(
    case: tuple[str, dict[str, object], list[str]],
) -> None:
    """An include-only matrix never merges two rows into one leg."""
    selector, job, expected = case
    actual = selected_labels(selector, job)
    assert actual == expected, (
        f"{selector} must resolve to one value per include row {expected!r}, "
        f"got {actual!r} from {job!r}"
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
@given(case=_axis_case(), value=_LABEL)
def test_a_bare_label_is_never_resolved_through_the_matrix(
    case: tuple[str, dict[str, object], list[str]], value: str
) -> None:
    """A literal names itself even when the job's matrix holds the same text."""
    _, job, _ = case
    actual = selected_labels(value, job)
    assert actual == [value], (
        f"the bare label {value!r} must pass through unchanged, got {actual!r}"
    )


@settings(max_examples=200, derandomize=True, deadline=None)
@given(literal=st.text(max_size=24))
def test_a_quoted_literal_round_trips_through_the_quote_escape(literal: str) -> None:
    """A literal reads back as the text it was escaped from, quotes included."""
    escaped = literal.replace("'", "''")
    actual = static_label("${{ '" + escaped + "' }}")
    expected = literal or None
    assert actual == expected, (
        f"the literal {literal!r}, written escaped as {escaped!r}, should read "
        f"back as {expected!r}, got {actual!r}"
    )


def test_an_empty_literal_selects_no_label() -> None:
    """An empty literal is not a runner label, and neither is a nameless axis."""
    assert static_label("${{ '' }}") is None, (
        "an empty literal must not become an empty runner label"
    )
    assert matrix_axis("${{ matrix. }}") is None, (
        "an axis expression with no axis name must not resolve"
    )


@settings(max_examples=200, derandomize=True, deadline=None)
@given(literal=st.text(min_size=1, max_size=16), decoration=_VISIBLE_TEXT)
def test_a_literal_inside_larger_text_is_not_a_static_label(
    literal: str, decoration: str
) -> None:
    """A label literal resolves only when it is the whole value."""
    expression = "${{ '" + literal.replace("'", "''") + "' }}"
    for composed in (decoration + expression, expression + decoration):
        assert static_label(composed) is None, (
            f"{composed!r} mixes a literal with other text, so it selects no "
            "single label"
        )


@settings(max_examples=200, derandomize=True, deadline=None)
@given(axis=_AXIS, other=_AXIS)
def test_a_matrix_axis_is_read_only_from_a_whole_expression_naming_it(
    axis: str, other: str
) -> None:
    """Composed text that contains a selector is not itself a selector."""
    selector = _selector(axis)
    assert matrix_axis(selector) == axis, (
        f"the whole expression {selector!r} should read axis {axis!r}"
    )
    composed = [
        f"{selector} {_selector(other)}",
        f"{selector}-suffix",
        f"prefix-{selector}",
        f"${{{{ matrix.{axis} || 'fallback' }}}}",
        f"${{{{ matrix.{axis}.field }}}}",
        f"${{{{ matrix.{axis} }}}}-${{{{ matrix.{other} }}}}",
        f"${{{{ inputs.{axis} }}}}",
        axis,
    ]
    for value in composed:
        assert matrix_axis(value) is None, (
            f"{value!r} does not consist of one matrix expression, so it must "
            "not resolve through the matrix"
        )
