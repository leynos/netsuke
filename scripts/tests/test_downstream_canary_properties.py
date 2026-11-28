"""Check the canary runner's argument splitting, status vocabulary, and assignments.

The downstream canary records its outcome in a bounded provenance record, so
three inputs must stay inside closed shapes whatever a caller passes: list
arguments split into clean values, unknown recorded statuses collapse to the
vocabulary's ``not_run``, and ``NAME=value`` selectors are accepted only for
variable names. Each property draws arbitrary text and compares the result
with the rule the code documents.

The module imports the runner's parser from ``scripts``, so run it with
``PYTHONPATH=scripts``. The parser's refusal names the assignment's position
and never echoes its text, which the assignment properties assert exactly.
"""

import re
import string

from downstream_canary_arguments import split_values
from downstream_canary_provenance import NOT_RUN, STATUSES, bounded_status
from hypothesis import given, settings
from hypothesis import strategies as st
from run_downstream_canary import SELECTOR_NAME, parse_assignments

#: Non-ASCII spaces that ``str.split()`` treats as separators. They are built
#: with ``chr`` so the source does not hide look-alike characters.
UNICODE_SPACES = (chr(0x00A0), chr(0x2003))
#: Characters that exercise every separator, including the Unicode spaces the
#: whitespace split treats as separators.
WHITESPACE_HEAVY = "ab \n\t\r\x0b\x0c\x1c" + "".join(UNICODE_SPACES)
PART_TEXT = st.one_of(
    st.text(max_size=24),
    st.text(alphabet=WHITESPACE_HEAVY, max_size=24),
)
SEPARATORS = st.sampled_from(["\n", None])
#: A variable name as an identifier, independent of the runner's regular
#: expression, so the expected verdict does not depend on the code under test.
VARIABLE_NAME = re.compile(r"[A-Z_][A-Z0-9_]*")
#: Name characters without a newline, so the generated names match the
#: documented rule rather than the trailing-newline quirk checked separately.
NAME_CHARACTERS = string.ascii_letters + string.digits + "_-. \t"
ANY_NAME = st.text(alphabet=NAME_CHARACTERS, max_size=12)
VALUE = st.text(alphabet=string.ascii_letters + string.digits + "=_ \n", max_size=12)
VALID_NAME = st.from_regex(r"[A-Z_][A-Z0-9_]{0,11}", fullmatch=True)
INVALID_NAME = ANY_NAME.filter(lambda name: VARIABLE_NAME.fullmatch(name) is None)
#: An assignment whose name is invalid, or that has no ``=`` at all.
INVALID_PAIR = st.one_of(
    st.tuples(INVALID_NAME, VALUE).map(lambda pair: f"{pair[0]}={pair[1]}"),
    ANY_NAME,
)
#: Recorded values of every shape a state file might carry.
STATUS_CANDIDATE = st.one_of(
    st.none(),
    st.integers(),
    st.booleans(),
    st.text(max_size=12),
    st.lists(
        st.one_of(
            st.none(),
            st.integers(),
            st.text(max_size=8),
            st.sampled_from(sorted(STATUSES)),
        ),
        max_size=3,
    ),
)


def _expected_parts(values: list[str], separator: str | None) -> list[str]:
    """Return the stripped, non-empty parts of each value, in order."""
    if separator is None:
        return [word for value in values for word in value.split()]
    return [
        line.strip()
        for value in values
        for line in value.split(separator)
        if line.strip()
    ]


def _is_separator_free(part: str, separator: str | None) -> bool:
    """Return whether a part carries no separator character of its kind."""
    if separator is None:
        return not any(character.isspace() for character in part)
    return separator not in part


def _outcome(pairs: list[str]) -> dict[str, str] | str:
    """Return the parsed selectors, or the message the parser refused them with."""
    try:
        return parse_assignments(pairs)
    except ValueError as error:
        return str(error)


@settings(max_examples=300, deadline=None, derandomize=True)
@given(values=st.lists(PART_TEXT, max_size=5), separator=SEPARATORS)
def test_split_values_returns_the_stripped_non_empty_parts_in_order(
    values: list[str], separator: str | None
) -> None:
    """The output is every stripped, non-empty part of every value, in order."""
    parts = split_values(values, separator)
    assert parts == _expected_parts(values, separator), (
        f"split_values({values!r}, {separator!r}) must yield the flattened "
        "stripped non-empty parts in order"
    )


@settings(max_examples=300, deadline=None, derandomize=True)
@given(values=st.lists(PART_TEXT, max_size=5), separator=SEPARATORS)
def test_split_values_items_are_non_empty_stripped_and_separator_free(
    values: list[str], separator: str | None
) -> None:
    """Every item is non-empty, stripped, and free of the separator's characters."""
    parts = split_values(values, separator)
    for part in parts:
        assert part, f"split_values({values!r}) must not yield an empty item"
        assert part == part.strip(), f"item {part!r} must already be stripped"
        assert _is_separator_free(part, separator), (
            f"item {part!r} must not contain the separator {separator!r}"
        )


@settings(max_examples=200, deadline=None, derandomize=True)
@given(values=st.lists(PART_TEXT, max_size=5), separator=SEPARATORS)
def test_split_values_is_idempotent_on_its_own_output(
    values: list[str], separator: str | None
) -> None:
    """Splitting already-split items again returns them unchanged."""
    parts = split_values(values, separator)
    assert split_values(parts, separator) == parts, (
        f"re-splitting {parts!r} must leave the items unchanged"
    )


@settings(max_examples=300, deadline=None, derandomize=True)
@given(value=STATUS_CANDIDATE)
def test_bounded_status_always_returns_a_member_of_the_vocabulary(
    value: object,
) -> None:
    """Any recorded value maps to a status in the closed vocabulary."""
    status = bounded_status(value)
    assert status in STATUSES, f"{value!r} mapped to {status!r} outside STATUSES"
    expected = value if isinstance(value, str) and value in STATUSES else NOT_RUN
    assert status == expected, f"{value!r} must map to {expected!r}, not {status!r}"


@settings(max_examples=50, deadline=None, derandomize=True)
@given(status=st.sampled_from(sorted(STATUSES)))
def test_bounded_status_keeps_every_known_status(status: str) -> None:
    """A recorded status from the vocabulary is returned unchanged."""
    assert bounded_status(status) == status, (
        f"the known status {status!r} must survive bounding"
    )


@settings(max_examples=300, deadline=None, derandomize=True)
@given(name=ANY_NAME, value=VALUE)
def test_assignments_are_accepted_exactly_for_variable_names(
    name: str, value: str
) -> None:
    """An assignment is accepted precisely when its name is a variable name."""
    outcome = _outcome([f"{name}={value}"])
    if VARIABLE_NAME.fullmatch(name) is not None:
        assert outcome == {name: value}, (
            f"the variable name {name!r} must be accepted with its value"
        )
    else:
        assert outcome == "environment assignment 1 is not NAME=value", (
            f"the name {name!r} is not a variable name and must be refused"
        )


@settings(max_examples=300, deadline=None, derandomize=True)
@given(prefix=st.lists(VALID_NAME, max_size=3), invalid=INVALID_PAIR)
def test_refusal_names_the_position_of_the_first_bad_assignment(
    prefix: list[str], invalid: str
) -> None:
    """A refused assignment is named by its position and its text is never echoed."""
    pairs = [*(f"{name}=x" for name in prefix), invalid]
    position = len(prefix) + 1
    assert _outcome(pairs) == (
        f"environment assignment {position} is not NAME=value"
    ), f"assignment {position} must be refused by position only"


@settings(max_examples=100, deadline=None, derandomize=True)
@given(name=VALID_NAME)
def test_selector_name_pattern_agrees_with_the_variable_name_rule(name: str) -> None:
    """The runner's selector pattern accepts every generated variable name."""
    assert SELECTOR_NAME.fullmatch(name) is not None, (
        f"the variable name {name!r} must match SELECTOR_NAME"
    )


@settings(max_examples=50, deadline=None, derandomize=True)
@given(name=VALID_NAME)
def test_a_name_with_a_trailing_newline_is_refused(name: str) -> None:
    """A variable name followed by a newline is not a variable name."""
    outcome = _outcome([f"{name}\n=x"])
    assert isinstance(outcome, str), (
        f"{name!r} followed by a newline must be refused, not accepted as {outcome!r}"
    )
