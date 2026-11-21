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
from runner_matrix import caller_runner_labels, selected_labels

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


def test_an_excluded_leg_does_not_report_its_label() -> None:
    """A label used only by a leg `exclude` removes must be absent.

    The inventory asks which runners a job can use, and an excluded leg never
    runs, so reporting its label would demand a registration nothing needs.
    """
    job: dict[str, object] = {
        "strategy": {
            "matrix": {
                "os": ["ubuntu-latest", "private-runner"],
                "exclude": [{"os": "private-runner"}],
            }
        }
    }
    assert selected_labels(UNRESOLVABLE, job) == ["ubuntu-latest"], (
        "the excluded private-runner leg must not reach the label inventory"
    )


def test_an_include_can_add_a_label_back_after_exclusion() -> None:
    """An include row may recreate a leg the base product excluded.

    GitHub applies `include` after `exclude`, and a row that matches no
    surviving original leg becomes a new one, so its label is in use again.
    """
    job: dict[str, object] = {
        "strategy": {
            "matrix": {
                "os": ["ubuntu-latest", "private-runner"],
                "exclude": [{"os": "private-runner"}],
                "include": [{"os": "private-runner"}],
            }
        }
    }
    assert selected_labels(UNRESOLVABLE, job) == [
        "ubuntu-latest",
        "private-runner",
    ], "an include row must be able to add an excluded label back"


def test_an_exclude_removes_a_leg_only_when_every_pair_matches() -> None:
    """A partial `exclude` row must not remove a leg it only partly names.

    An exclude row is a conjunction. Matching it on any one pair would drop
    legs whose other axis values the row never mentioned.
    """
    job: dict[str, object] = {
        "strategy": {
            "matrix": {
                "os": ["linux", "windows"],
                "arch": ["x64", "arm64"],
                "exclude": [{"os": "linux", "arch": "arm64"}],
            }
        }
    }
    assert selected_labels(UNRESOLVABLE, job) == ["linux", "windows"], (
        "a partial exclude row must not remove either os label"
    )


def test_successive_includes_report_only_the_final_label() -> None:
    """Later include rows override earlier added values on the same leg.

    Both rows join the one original combination, so only the second runner is
    selected; reporting the first would name a runner no leg uses.
    """
    job: dict[str, object] = {
        "strategy": {
            "matrix": {
                "os": ["ubuntu-latest"],
                "include": [{"runner": "first-runner"}, {"runner": "second-runner"}],
            }
        }
    }
    assert selected_labels("${{ matrix.runner }}", job) == ["second-runner"], (
        "only the final include value on a leg may reach the inventory"
    )


@pytest.mark.parametrize(
    "selector",
    [
        pytest.param("${{ matrix.os }}", id="lowercase"),
        pytest.param("${{ matrix.OS }}", id="uppercase"),
        pytest.param("${{ matrix.Os }}", id="mixed"),
    ],
)
def test_axis_lookup_is_case_insensitive(selector: str) -> None:
    """A selector resolves an axis however it spells the key's case.

    GitHub evaluates `matrix.<axis>` case-insensitively, so a selector must
    read an axis or include key written in another case rather than dropping
    the labels it holds.
    """
    job: dict[str, object] = {
        "strategy": {
            "matrix": {"OS": ["first-runner"], "include": [{"os": "second-runner"}]}
        }
    }
    assert selected_labels(selector, job) == [
        "first-runner",
        "second-runner",
    ], f"{selector} must resolve the OS axis and its case-variant include row"


def test_an_include_key_matches_a_base_axis_case_insensitively() -> None:
    """An include row naming a base axis in another case joins only matches.

    Treating `OS` as an unrelated added key would let the row overwrite the
    original `os` value on every leg; it must instead disagree with `linux`
    and become its own leg, leaving the original's later runner override.
    """
    job: dict[str, object] = {
        "strategy": {
            "matrix": {
                "os": ["linux"],
                "include": [
                    {"OS": "windows", "runner": "new-leg"},
                    {"runner": "base-leg"},
                ],
            }
        }
    }
    assert selected_labels("${{ matrix.runner }}", job) == [
        "base-leg",
        "new-leg",
    ], "the case-variant OS row must create a leg rather than overwrite os"


def test_an_exclude_key_matches_its_axis_case_insensitively() -> None:
    """An exclude row naming an axis in another case still removes its leg."""
    job: dict[str, object] = {
        "strategy": {"matrix": {"OS": ["a", "b"], "exclude": [{"os": "a"}]}}
    }
    assert selected_labels(UNRESOLVABLE, job) == ["b"], (
        "the exclude row must match the OS axis despite its lowercase key"
    )


def test_an_include_only_matrix_keeps_one_leg_per_row() -> None:
    """Include-only matrices create a leg per row, never merging the rows.

    The MSI gate names one row per architecture. If later rows merged into an
    earlier row's leg, an architecture could silently lose its leg.
    """
    job: dict[str, object] = {
        "strategy": {
            "matrix": {
                "include": [
                    {"runner": "first-runner"},
                    {"arch": "x64"},
                    {"runner": "second-runner"},
                ]
            }
        }
    }
    assert selected_labels("${{ matrix.runner }}", job) == [
        "first-runner",
        "second-runner",
    ], "each include-only row must remain its own leg"


def test_a_local_caller_runner_input_reaches_the_inventory() -> None:
    """A runner passed to a local reusable workflow resolves from its matrix.

    `release.build-macos` has no `runs-on`: it passes `${{ matrix.runner }}` to
    `build-and-package.yml`, whose job reads `${{ inputs.runner }}`. Without
    reading the caller's input, an unregistered label on an Arm64 leg would
    escape the registry comparison.
    """
    job: dict[str, object] = {
        "uses": "./.github/workflows/build-and-package.yml",
        "strategy": {
            "matrix": {
                "include": [
                    {"target": "x86_64-apple-darwin", "runner": "macos-15-intel"},
                    {
                        "target": "aarch64-apple-darwin",
                        "runner": "private-macos-runner",
                    },
                ]
            }
        },
        "with": {"runner": "${{ matrix.runner }}"},
    }
    assert caller_runner_labels(job) == [
        "macos-15-intel",
        "private-macos-runner",
    ], "a caller-selected runner must reach the label inventory"


@pytest.mark.parametrize(
    "job",
    [
        pytest.param({"with": {"runner": "private-runner"}}, id="no-uses"),
        pytest.param(
            {
                "uses": "leynos/shared-actions/.github/workflows/build.yml",
                "with": {"runner": "private-runner"},
            },
            id="a-remote-call",
        ),
    ],
)
def test_a_non_local_call_does_not_resolve_its_runner_input(
    job: dict[str, object],
) -> None:
    """Only a workflow checked in here has its runner input read."""
    labels = caller_runner_labels(job)
    assert not labels, f"a non-local call must contribute no label, got {labels!r}"
