"""Hold matrix-selector resolution to the labels the registry must see.

`runner_shape_test` derives the self-hosted labels in use from the workflows
and requires actionlint to register exactly those. A lane that chooses its
runner with `${{ matrix.os }}` holds its labels in the job's matrix, so a
resolution that reads no values, or the wrong ones, removes the label from the
inventory and the comparison passes over a runner nobody registered.

Run via ``make test-workflow-contracts``.
"""

import pytest
from actions_expressions import matrix_axis
from runner_matrix import selected_labels

UNRESOLVABLE = "${{ matrix.os }}"


@pytest.mark.parametrize(
    ("value", "axis"),
    [
        ("${{ matrix.os }}", "os"),
        ("${{ matrix.runner }}", "runner"),
        ("${{ inputs.runner }}", None),
        ("${{ matrix.os }}-${{ matrix.arch }}", None),
    ],
)
def test_only_a_whole_single_axis_expression_names_an_axis(
    value: str, axis: str | None
) -> None:
    """A selector that computes, mixes, or reads another context names none.

    Only a whole `matrix.<axis>` expression can be resolved against a job's
    matrix, so anything else must stay whole rather than be half-read.
    """
    assert matrix_axis(value) == axis, (
        f"{value!r} must read the axis {axis!r}, got {matrix_axis(value)!r}"
    )


@pytest.mark.parametrize(
    ("matrix", "expected"),
    [
        pytest.param({"os": ["a", "b"]}, ["a", "b"], id="a-plain-axis-list"),
        pytest.param(
            {"include": [{"os": "c"}, {"os": "d"}]}, ["c", "d"], id="include-entries"
        ),
        pytest.param(
            {"os": ["a"], "include": [{"os": "c"}]},
            ["a", "c"],
            id="a-list-and-its-includes",
        ),
        pytest.param(
            {"runner": ["private-runner"]},
            [UNRESOLVABLE],
            id="another-axis-altogether",
        ),
    ],
)
def test_a_selector_resolves_against_its_own_axis_only(
    matrix: dict[str, object], expected: list[str]
) -> None:
    """Every leg of the named axis counts, and no other axis's legs do.

    A `runner` include read for any selector would let a matrix label in on an
    axis the job never reads; reading nothing would drop the label entirely.
    """
    # Annotated because a nested literal infers a narrower mapping, and a dict
    # is invariant in its value type, so the call would not typecheck.
    job: dict[str, object] = {"strategy": {"matrix": matrix}}
    assert selected_labels(UNRESOLVABLE, job) == expected, (
        f"a selector over {matrix!r} must resolve to {expected!r}, "
        f"got {selected_labels(UNRESOLVABLE, job)!r}"
    )


@pytest.mark.parametrize(
    ("job", "expected"),
    [
        pytest.param({}, [UNRESOLVABLE], id="no-strategy"),
        pytest.param({"strategy": None}, [UNRESOLVABLE], id="no-matrix"),
        pytest.param({"strategy": {"matrix": {}}}, [UNRESOLVABLE], id="no-such-axis"),
    ],
)
def test_an_unreadable_selector_stays_visible(
    job: dict[str, object], expected: list[str]
) -> None:
    """An unresolvable selector is returned as itself, never as nothing.

    The caller counts what it cannot read as a self-hosted label, so returning
    an empty list here is exactly the fail-open the registry comparison exists
    to prevent.
    """
    assert selected_labels(UNRESOLVABLE, job) == expected, (
        f"{job!r} cannot resolve the selector, so it must be returned as itself"
    )


def test_a_fixed_literal_selects_exactly_that_label() -> None:
    """A quoted literal and a bare label both name one label, with no matrix."""
    assert selected_labels("${{ 'private-runner' }}", {}) == ["private-runner"], (
        "a literal expression must resolve to the label it selects"
    )
    assert selected_labels("ubuntu-latest", {}) == ["ubuntu-latest"], (
        "a bare label must pass through unchanged"
    )
