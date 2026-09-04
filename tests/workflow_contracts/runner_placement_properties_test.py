"""Generate valid and invalid runner-placement and cache configurations.

The checked-in workflow suite validates repository YAML directly. These
properties exercise the same pure validators against bounded mutations for
setup order, duplicate setup, missing setup, runner-platform drift, duplicated
cache ownership, and oversubscribed worker counts.

Run via ``make test-workflow-contracts``.
"""

import pytest
from hypothesis import example, given, settings
from hypothesis import strategies as st
from runner_placement_invariants import (
    NINJA_ACTION,
    UBICLOUD_ASSIGNMENT_KEYS,
    WINDOWS_PACKAGE_ACTION,
    WINDOWS_PATH_FRAGMENTS,
    WINDOWS_PATH_STEP_NAME,
    has_required_runner_assignments,
    has_single_cache_owner,
    is_bounded_worker_count,
    is_trunk_only_save,
    is_valid_ninja_sequence,
    is_valid_windows_tool_path_sequence,
)
from runner_placement_mutations import (
    mutate_runner_assignments,
    mutate_save_condition,
    mutate_worker_flags,
)

NINJA_CONSUMER = "Consume Ninja"
IRRELEVANT_STEP_NAMES = ("Checkout", "Setup Rust", "Upload artefact")
SEQUENCE_MUTATIONS = ("valid", "missing", "duplicate", "after-consumer")
RUNNER_MUTATIONS = (
    "valid",
    "github-hosted",
    "wrong-ubicloud-image",
    "swapped-platforms",
    "intel-macos-replaced",
)
SEQUENCE_KINDS = ("ninja", "windows")
CACHE_MUTATIONS = ("valid", "duplicate-path")
WORKER_MUTATIONS = ("valid", "oversubscribed", "zero", "unbounded")
SAVE_MUTATIONS = (
    "valid",
    "any-push",
    "any-branch",
    "unconditional",
    "disjunctive",
    "parenthesised-disjunctive",
)

#: Disjoint path sets, one per cache owner, as the workflows declare them.
CACHE_OWNERS = (
    ("registry", "~/.cargo/registry"),
    ("registry", "~/.cargo/git"),
    ("tools", "~/.cargo/bin"),
    ("tools", "~/.local/bin"),
    ("whitaker", "~/.local/share"),
)


def _irrelevant_steps(names: list[str]) -> list[dict[str, object]]:
    """Build inert workflow steps from bounded generated names."""
    return [{"name": name, "run": "true"} for name in names]


def _ninja_sequence(
    before: list[str], between: list[str], after: list[str]
) -> list[dict[str, object]]:
    """Build a valid pinned Ninja setup and consumer sequence."""
    return [
        *_irrelevant_steps(before),
        {"name": "Install Ninja", "uses": NINJA_ACTION},
        *_irrelevant_steps(between),
        {"name": NINJA_CONSUMER, "run": "ninja --version"},
        *_irrelevant_steps(after),
    ]


def _mutate_ninja_sequence(
    steps: list[dict[str, object]], mutation: str
) -> list[dict[str, object]]:
    """Apply one required Ninja-sequence mutation."""
    setup: dict[str, object] = {"name": "Install Ninja", "uses": NINJA_ACTION}
    setup_index = next(
        index for index, step in enumerate(steps) if step.get("uses") == NINJA_ACTION
    )
    consumer_index = next(
        index for index, step in enumerate(steps) if step.get("name") == NINJA_CONSUMER
    )
    match mutation:
        case "missing":
            steps.pop(setup_index)
        case "duplicate":
            steps.insert(consumer_index, setup)
        case "after-consumer":
            steps.pop(setup_index)
            consumer_index = next(
                index
                for index, step in enumerate(steps)
                if step.get("name") == NINJA_CONSUMER
            )
            steps.insert(consumer_index + 1, setup)
    return steps


def _windows_path_step() -> dict[str, object]:
    """Build the required known-folder PowerShell path step."""
    return {
        "name": WINDOWS_PATH_STEP_NAME,
        "if": "inputs.platform == 'windows'",
        "shell": "pwsh",
        "run": "\n".join(WINDOWS_PATH_FRAGMENTS),
    }


def _windows_sequence(
    before: list[str], between: list[str], after: list[str]
) -> list[dict[str, object]]:
    """Build a valid Windows path setup and packaging sequence."""
    return [
        *_irrelevant_steps(before),
        _windows_path_step(),
        *_irrelevant_steps(between),
        {
            "name": "Build Windows installer package",
            "uses": f"{WINDOWS_PACKAGE_ACTION}pinned-sha",
        },
        *_irrelevant_steps(after),
    ]


def _mutate_windows_sequence(
    steps: list[dict[str, object]], mutation: str
) -> list[dict[str, object]]:
    """Apply one required Windows path-sequence mutation."""
    path_index = next(
        index
        for index, step in enumerate(steps)
        if step.get("name") == WINDOWS_PATH_STEP_NAME
    )
    package_index = next(
        index
        for index, step in enumerate(steps)
        if str(step.get("uses", "")).startswith(WINDOWS_PACKAGE_ACTION)
    )
    match mutation:
        case "missing":
            steps.pop(path_index)
        case "duplicate":
            steps.insert(package_index, _windows_path_step())
        case "after-consumer":
            path_step = steps.pop(path_index)
            package_index = next(
                index
                for index, step in enumerate(steps)
                if str(step.get("uses", "")).startswith(WINDOWS_PACKAGE_ACTION)
            )
            steps.insert(package_index + 1, path_step)
    return steps


def _mutate_cache_owners(mutation: str, selected: int) -> list[tuple[str, str]]:
    """Apply one bounded cache-ownership mutation to the valid layout."""
    owners = list(CACHE_OWNERS)
    if mutation == "duplicate-path":
        owners.append(("sccache", owners[selected][1]))
    return owners


def _generated_sequence_is_valid(
    sequence_kind: str,
    mutation: str,
    segments: tuple[list[str], list[str], list[str]],
) -> tuple[bool, list[dict[str, object]]]:
    """Build, mutate, and validate one generated setup sequence."""
    before, between, after = segments
    if sequence_kind == "ninja":
        steps = _mutate_ninja_sequence(
            _ninja_sequence(before, between, after), mutation
        )
        return is_valid_ninja_sequence(steps, NINJA_CONSUMER), steps

    steps = _mutate_windows_sequence(
        _windows_sequence(before, between, after), mutation
    )
    return is_valid_windows_tool_path_sequence(steps), steps


@settings(max_examples=48, derandomize=True, deadline=None)
@example(sequence_kind="ninja", mutation="missing", segments=([], [], []))
@example(sequence_kind="ninja", mutation="duplicate", segments=([], [], []))
@example(
    sequence_kind="ninja",
    mutation="after-consumer",
    segments=([], [], []),
)
@example(sequence_kind="windows", mutation="missing", segments=([], [], []))
@example(sequence_kind="windows", mutation="duplicate", segments=([], [], []))
@example(
    sequence_kind="windows",
    mutation="after-consumer",
    segments=([], [], []),
)
@given(
    sequence_kind=st.sampled_from(SEQUENCE_KINDS),
    mutation=st.sampled_from(SEQUENCE_MUTATIONS),
    segments=st.tuples(
        st.lists(st.sampled_from(IRRELEVANT_STEP_NAMES), max_size=2),
        st.lists(st.sampled_from(IRRELEVANT_STEP_NAMES), max_size=2),
        st.lists(st.sampled_from(IRRELEVANT_STEP_NAMES), max_size=2),
    ),
)
def test_generated_setup_sequences_accept_only_valid_order(
    sequence_kind: str,
    mutation: str,
    segments: tuple[list[str], list[str], list[str]],
) -> None:
    """Accept only valid Ninja and Windows prerequisite setup order."""
    is_valid, steps = _generated_sequence_is_valid(sequence_kind, mutation, segments)
    assert is_valid is (mutation == "valid"), (
        f"sequence_kind={sequence_kind!r}, mutation={mutation!r}, steps={steps!r}"
    )


@settings(max_examples=40, derandomize=True, deadline=None)
@example(mutation="github-hosted", selected_key="ci.build-test")
@example(mutation="wrong-ubicloud-image", selected_key="ci.build-test")
@example(
    mutation="wrong-ubicloud-image",
    selected_key="netsukefile-test.netsukefile",
)
@example(mutation="swapped-platforms", selected_key="release.build-linux")
@example(mutation="intel-macos-replaced", selected_key="ci.kani-smoke")
@given(
    mutation=st.sampled_from(RUNNER_MUTATIONS),
    selected_key=st.sampled_from(UBICLOUD_ASSIGNMENT_KEYS),
)
def test_generated_runner_assignments_accept_only_platform_contract(
    mutation: str, selected_key: str
) -> None:
    """Accept only the required Ubicloud and GitHub-hosted assignments."""
    assignments = mutate_runner_assignments(mutation, selected_key)
    assert has_required_runner_assignments(assignments) is (mutation == "valid"), (
        f"mutation={mutation!r}, selected_key={selected_key!r}, "
        f"assignments={assignments!r}"
    )


@settings(max_examples=32, derandomize=True, deadline=None)
@example(mutation="duplicate-path", selected=0)
@example(mutation="duplicate-path", selected=len(CACHE_OWNERS) - 1)
@given(
    mutation=st.sampled_from(CACHE_MUTATIONS),
    selected=st.integers(min_value=0, max_value=len(CACHE_OWNERS) - 1),
)
def test_generated_cache_layouts_reject_a_second_owner(
    mutation: str, selected: int
) -> None:
    """Accept a cache layout only while every path has exactly one owner."""
    owners = _mutate_cache_owners(mutation, selected)
    assert has_single_cache_owner(owners) is (mutation == "valid"), (
        f"mutation={mutation!r}, selected={selected!r}, owners={owners!r}"
    )


@settings(max_examples=32, derandomize=True, deadline=None)
@example(mutation="oversubscribed", vcpus=2)
@example(mutation="unbounded", vcpus=2)
@example(mutation="zero", vcpus=4)
@given(
    mutation=st.sampled_from(WORKER_MUTATIONS),
    vcpus=st.sampled_from((2, 4)),
)
def test_generated_worker_counts_stay_within_the_lane(
    mutation: str, vcpus: int
) -> None:
    """Accept worker counts only while they fit the placed shape."""
    flags = mutate_worker_flags(mutation, vcpus)
    assert is_bounded_worker_count(vcpus, flags) is (mutation == "valid"), (
        f"mutation={mutation!r}, vcpus={vcpus!r}, flags={flags!r}"
    )


@settings(max_examples=24, derandomize=True, deadline=None)
@example(mutation="any-push")
@example(mutation="any-branch")
@example(mutation="unconditional")
@example(mutation="disjunctive")
@example(mutation="parenthesised-disjunctive")
@given(mutation=st.sampled_from(SAVE_MUTATIONS))
def test_generated_save_conditions_require_a_trunk_push(mutation: str) -> None:
    """Accept a save condition only while it names a push on the trunk."""
    condition = mutate_save_condition(mutation)
    assert is_trunk_only_save(condition) is (mutation == "valid"), (
        f"mutation={mutation!r}, condition={condition!r}"
    )


@pytest.mark.parametrize(
    "condition",
    [
        pytest.param(
            "github.event_name == 'push' || github.ref == 'refs/heads/main'",
            id="bare",
        ),
        pytest.param(
            "(github.event_name == 'push' || github.ref == 'refs/heads/main')",
            id="parenthesised",
        ),
        pytest.param(
            "github.event_name == 'push' && (github.ref == 'refs/heads/main' "
            "|| github.ref == 'refs/heads/release')",
            id="nested-in-conjunction",
        ),
    ],
)
def test_disjunctive_save_conditions_are_rejected(condition: str) -> None:
    """Reject an OR of the push and main predicates at any nesting depth.

    Each of these authorises a save that neither predicate alone should. The
    parenthesised form is the reason the check cannot look only at the top
    level: it means what the bare form means while presenting no top-level
    `||` at all.
    """
    assert is_trunk_only_save(condition) is False, f"condition={condition!r}"
