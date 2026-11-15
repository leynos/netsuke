"""Prove the release-path contract rejects each consumer-smoke regression.

Every case here starts from the checked-in workflow and action, applies one
labelled regression to a copy, and requires the contract to name the scenario
and property that regression breaks. A contract that stopped reporting one of
them would leave the smoke free to hold a credential, build from source, or run
against a release that was never finalized, so each label is asserted rather
than the suite's overall exit status.

Run via ``make test-workflow-contracts``.
"""

import copy
import dataclasses

import pytest
from action_run_sources import action_sources
from release_publish_path_consumer_mutations import (
    CONSUMER_SMOKE_EXPECTED_VIOLATIONS,
    MUTATION_LABELS,
    mutate_consumer_smoke,
)
from release_publish_path_consumer_smoke import (
    CONSUMER_SMOKE_ACTION_PATH,
    CONSUMER_SMOKE_JOB,
    check_smoke_compilation,
)
from release_publish_path_contract import (
    CALLER_WORKFLOW_PATH,
    check_release_publish_path,
)
from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    load_workflow,
    workflow_job,
)


@dataclasses.dataclass(frozen=True, slots=True)
class SmokeFixtures:
    """Hold the pristine documents every mutation is copied from.

    Attributes
    ----------
    release
        The parsed release workflow.
    caller
        The dry-run caller workflow, unchanged by these mutations.
    build
        The package workflow, unchanged by these mutations.
    action
        The parsed consumer-smoke manifest.
    """

    release: dict[str, object]
    caller: dict[str, object]
    build: dict[str, object]
    action: dict[str, object]


@pytest.fixture(scope="module")
def fixtures() -> SmokeFixtures:
    """Parse the release documents and the smoke manifest once."""
    return SmokeFixtures(
        release=load_workflow(RELEASE_WORKFLOW_PATH),
        caller=load_workflow(CALLER_WORKFLOW_PATH),
        build=load_workflow(PACKAGE_WORKFLOW_PATH),
        action=action_sources()[CONSUMER_SMOKE_ACTION_PATH],
    )


@pytest.mark.parametrize("label", MUTATION_LABELS, ids=MUTATION_LABELS)
def test_release_path_contract_rejects_each_regression(
    label: str, fixtures: SmokeFixtures
) -> None:
    """Require each mutation's affected scenario or invariant to fail by name."""
    release = copy.deepcopy(fixtures.release)
    action = copy.deepcopy(fixtures.action)
    mutate_consumer_smoke(label, release, action)

    violations = check_release_publish_path(
        release, fixtures.caller, fixtures.build, consumer_smoke_action=action
    )

    for scenario, contract in CONSUMER_SMOKE_EXPECTED_VIOLATIONS[label]:
        prefix = f"{contract}:" if scenario == "static" else f"{scenario}.{contract}:"
        assert any(violation.startswith(prefix) for violation in violations), (
            f"{label} should report {prefix!r}; got {violations!r}"
        )


def test_the_compilation_scan_finds_no_build_in_the_shipped_smoke(
    fixtures: SmokeFixtures,
) -> None:
    """Keep the compilation check live by clearing the real documents.

    The mutation that appends ``cargo install netsuke-build`` proves the check
    reacts to a build; this case proves it reacts to nothing else. Without it,
    a check that reported a violation unconditionally would pass every
    mutation above while making the smoke impossible to ship.
    """
    job = workflow_job(fixtures.release, CONSUMER_SMOKE_JOB)
    violations: list[str] = []

    check_smoke_compilation(job, fixtures.action, violations)

    assert violations == [], (
        f"the shipped smoke must not be reported as compiling; got {violations!r}"
    )
