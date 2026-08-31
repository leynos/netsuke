"""Generated transition-model tests for CI installer and lint sequencing.

The example-based workflow contract suites assert the checked-in workflow's
fragments directly. This module complements them by generating valid and
invalid transition sequences, so omissions, duplicate gates, incorrect setup
order, and missing PowerShell restoration cannot be mistaken for valid states.

Run via ``make test-workflow-contracts``.
"""

from hypothesis import example, given, settings
from hypothesis import strategies as st
from workflow_loading import job_steps, load_workflow

#: The Linux sequence that makes Nixie available before Mermaid validation.
LINUX_JOB = "build-test"
NIXIE_VALIDATION_STEPS = (
    "Setup uv",
    "Install Nixie",
    "Validate Mermaid diagrams",
)

#: The PowerShell transitions that preserve both Whitaker exit statuses and
#: the caller's location.
WHITAKER_STATE_TRANSITIONS = (
    "load root Dylint configuration",
    "lint netsuke-build",
    "guard root exit status",
    "enter test_support",
    "load test_support Dylint configuration",
    "lint test_support",
    "guard test_support exit status",
    "finally",
    "restore location",
)

#: Unrelated workflow steps used to vary generated transition sequences.
IRRELEVANT_WORKFLOW_STEPS = ("Checkout", "Setup Rust", "Format")


def is_exactly_once_in_order(steps: list[str], expected_steps: tuple[str, ...]) -> bool:
    """Return whether ``expected_steps`` occur once and in declaration order."""
    return all(steps.count(step) == 1 for step in expected_steps) and (
        [steps.index(step) for step in expected_steps]
        == sorted(steps.index(step) for step in expected_steps)
    )


def mutate_transition_sequence(
    expected_steps: tuple[str, ...],
    mutation: str,
    selected_step: str,
    irrelevant_steps: list[str],
) -> list[str]:
    """Build one valid or deliberately invalid generated transition sequence."""
    steps = [*irrelevant_steps, *expected_steps]
    match mutation:
        case "missing":
            steps.remove(selected_step)
        case "duplicate":
            steps.append(selected_step)
        case "misordered":
            first = steps.index(expected_steps[0])
            last = steps.index(expected_steps[-1])
            steps[first], steps[last] = steps[last], steps[first]
    return steps


def test_linux_job_orders_nixie_before_mermaid_validation() -> None:
    """The checked-in Linux workflow must follow the Nixie transition model."""
    steps = job_steps(load_workflow(), LINUX_JOB)
    step_names = [str(step.get("name", "")) for step in steps]
    assert is_exactly_once_in_order(step_names, NIXIE_VALIDATION_STEPS), (
        "Setup uv, Install Nixie, and Validate Mermaid diagrams must occur "
        f"exactly once in that order, got step order {step_names!r}"
    )


@settings(max_examples=24, deadline=None, derandomize=True)
@example(mutation="missing", selected_step="Install Nixie", irrelevant_steps=[])
@example(mutation="duplicate", selected_step="Install Nixie", irrelevant_steps=[])
@example(mutation="misordered", selected_step="Install Nixie", irrelevant_steps=[])
@given(
    mutation=st.sampled_from(("valid", "missing", "duplicate", "misordered")),
    selected_step=st.sampled_from(NIXIE_VALIDATION_STEPS),
    irrelevant_steps=st.lists(st.sampled_from(IRRELEVANT_WORKFLOW_STEPS), max_size=3),
)
def test_generated_nixie_sequence_preserves_installer_order(
    mutation: str,
    selected_step: str,
    irrelevant_steps: list[str],
) -> None:
    """Generated Nixie sequences accept only one correctly ordered install path."""
    steps = mutate_transition_sequence(
        NIXIE_VALIDATION_STEPS,
        mutation,
        selected_step,
        irrelevant_steps,
    )
    assert is_exactly_once_in_order(steps, NIXIE_VALIDATION_STEPS) is (
        mutation == "valid"
    ), (
        "Setup uv, Install Nixie, and Validate Mermaid diagrams must occur "
        f"exactly once in that order; mutation={mutation!r}, steps={steps!r}"
    )


@settings(max_examples=24, deadline=None, derandomize=True)
@example(
    mutation="missing",
    selected_transition="restore location",
    irrelevant_steps=[],
)
@example(
    mutation="duplicate",
    selected_transition="lint netsuke-build",
    irrelevant_steps=[],
)
@example(
    mutation="misordered",
    selected_transition="enter test_support",
    irrelevant_steps=[],
)
@given(
    mutation=st.sampled_from(("valid", "missing", "duplicate", "misordered")),
    selected_transition=st.sampled_from(WHITAKER_STATE_TRANSITIONS),
    irrelevant_steps=st.lists(st.sampled_from(IRRELEVANT_WORKFLOW_STEPS), max_size=3),
)
def test_generated_whitaker_transitions_preserve_location_and_failures(
    mutation: str,
    selected_transition: str,
    irrelevant_steps: list[str],
) -> None:
    """Generated Whitaker transitions require both exit guards and restoration."""
    transitions = mutate_transition_sequence(
        WHITAKER_STATE_TRANSITIONS,
        mutation,
        selected_transition,
        irrelevant_steps,
    )
    assert is_exactly_once_in_order(transitions, WHITAKER_STATE_TRANSITIONS) is (
        mutation == "valid"
    ), (
        "Whitaker must guard each native invocation, enter test_support only "
        "after root linting, and restore the location in finally; "
        f"mutation={mutation!r}, transitions={transitions!r}"
    )
