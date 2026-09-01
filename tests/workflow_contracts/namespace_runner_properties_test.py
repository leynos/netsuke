"""Generate valid and invalid Namespace runner workflow configurations.

The checked-in workflow suite validates repository YAML directly. These
properties exercise the same pure validators against bounded mutations for
setup order, duplicate setup, missing setup, and runner-platform drift.

Run via ``make test-workflow-contracts``.
"""

from hypothesis import example, given, settings
from hypothesis import strategies as st
from namespace_runner_invariants import (
    NAMESPACE_ASSIGNMENT_KEYS,
    NINJA_ACTION,
    REQUIRED_RUNNER_ASSIGNMENTS,
    WINDOWS_PACKAGE_ACTION,
    WINDOWS_PATH_FRAGMENTS,
    WINDOWS_PATH_STEP_NAME,
    has_required_runner_assignments,
    is_valid_ninja_sequence,
    is_valid_windows_tool_path_sequence,
)

NINJA_CONSUMER = "Consume Ninja"
IRRELEVANT_STEP_NAMES = ("Checkout", "Setup Rust", "Upload artefact")
SEQUENCE_MUTATIONS = ("valid", "missing", "duplicate", "after-consumer")
RUNNER_MUTATIONS = (
    "valid",
    "github-hosted",
    "wrong-namespace",
    "swapped-platforms",
    "intel-macos-replaced",
)
SEQUENCE_KINDS = ("ninja", "windows")


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


def _mutate_runner_assignments(mutation: str, selected_key: str) -> dict[str, str]:
    """Apply one bounded runner-assignment mutation to the valid mapping."""
    assignments = dict(REQUIRED_RUNNER_ASSIGNMENTS)
    expected = assignments[selected_key]
    match mutation:
        case "github-hosted":
            assignments[selected_key] = _github_hosted_runner_for(selected_key)
        case "wrong-namespace":
            assignments[selected_key] = next(
                runner
                for runner in REQUIRED_RUNNER_ASSIGNMENTS.values()
                if runner.startswith("namespace-profile-") and runner != expected
            )
        case "swapped-platforms":
            linux_key = "release.build-linux"
            windows_key = "release.build-windows"
            assignments[linux_key], assignments[windows_key] = (
                assignments[windows_key],
                assignments[linux_key],
            )
        case "intel-macos-replaced":
            assignments["release.macos.x86_64-apple-darwin"] = (
                "namespace-profile-netsuke-macos-arm64"
            )
    return assignments


def _github_hosted_runner_for(selected_key: str) -> str:
    """Choose a bounded hosted-runner mutation for the selected platform."""
    if "windows" in selected_key:
        return "windows-latest"
    if "macos" in selected_key:
        return "macos-15"
    return "ubuntu-latest"


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
@example(mutation="github-hosted", selected_key="release.build-windows")
@example(
    mutation="github-hosted",
    selected_key="release.macos.aarch64-apple-darwin",
)
@example(mutation="wrong-namespace", selected_key="release.build-windows")
@example(mutation="swapped-platforms", selected_key="release.build-linux")
@example(
    mutation="intel-macos-replaced",
    selected_key="release.macos.aarch64-apple-darwin",
)
@given(
    mutation=st.sampled_from(RUNNER_MUTATIONS),
    selected_key=st.sampled_from(NAMESPACE_ASSIGNMENT_KEYS),
)
def test_generated_runner_assignments_accept_only_platform_contract(
    mutation: str, selected_key: str
) -> None:
    """Accept only the required Namespace and retained runner assignments."""
    assignments = _mutate_runner_assignments(mutation, selected_key)
    assert has_required_runner_assignments(assignments) is (mutation == "valid"), (
        f"mutation={mutation!r}, selected_key={selected_key!r}, "
        f"assignments={assignments!r}"
    )
