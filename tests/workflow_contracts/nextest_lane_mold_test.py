"""Contract: every Linux job running the nextest suite installs the pinned mold.

The rule lives in ``nextest_lane_rules.py``. These tests run it over this
repository's workflows, then over copies mutated the way a later edit could,
and assert the clause meant to catch each edit does. The narrow cases show the
rule leaves alone the jobs it has no claim on.

Run via ``make test-workflow-contracts``.
"""

import copy
import typing as typ

import pytest
from nextest_lane_rules import (
    is_linux_job,
    nextest_goals,
    nextest_lane_violations,
    runner_labels,
    suite_lanes,
)
from workflow_loading import (
    MAKEFILE_PATH,
    REPO_ROOT,
    all_workflow_documents,
    require_list,
    require_mapping,
)

WORKFLOW_DIR = REPO_ROOT / ".github" / "workflows"
PUBLISHER = "coverage-main.yml"
PUBLISHER_JOB = "coverage-upload"

type Documents = dict[str, dict[str, object]]


@pytest.fixture
def makefile() -> str:
    """Return the Makefile text the goal derivation reads."""
    return MAKEFILE_PATH.read_text(encoding="utf-8")


@pytest.fixture
def documents() -> Documents:
    """Return a private copy of the workflows for one test to mutate."""
    return copy.deepcopy(all_workflow_documents(WORKFLOW_DIR))


def _jobs(documents: Documents, name: str) -> dict[str, object]:
    """Return one workflow's jobs mapping, for mutation in place."""
    return require_mapping(documents[name].get("jobs"), f"{name} jobs")


def _steps(documents: Documents, name: str, job: str) -> list[dict[str, object]]:
    """Return one job's steps, for mutation in place."""
    mapping = require_mapping(_jobs(documents, name).get(job), f"{name} {job}")
    steps = require_list(mapping.get("steps"), f"{name} {job} steps")
    return typ.cast("list[dict[str, object]]", steps)


def _install(steps: list[dict[str, object]]) -> dict[str, object]:
    """Return the step running `make install-build-tools`."""
    return next(s for s in steps if s.get("run") == "make install-build-tools")


def _reports(documents: Documents, makefile: str, fragment: str) -> None:
    """Fail unless the rule reports a violation containing ``fragment``."""
    found = nextest_lane_violations(documents, makefile)
    assert any(fragment in problem for problem in found), (
        f"expected a violation naming {fragment!r}, got {found}"
    )


def _clean(documents: Documents, makefile: str) -> None:
    """Fail unless the rule reports nothing, naming what it did report."""
    found = nextest_lane_violations(documents, makefile)
    assert not found, f"expected no violations, got {found}"


def test_every_linux_suite_lane_installs_the_build_standard(
    documents: Documents, makefile: str
) -> None:
    """The repository as it stands satisfies the rule."""
    found = nextest_lane_violations(documents, makefile)
    assert not found, f"expected no violations, got {found}"


def test_the_known_suite_lanes_are_recognised(
    documents: Documents, makefile: str
) -> None:
    """The derivation finds the lanes it guards, so it cannot pass on nothing."""
    lanes = set(suite_lanes(documents, makefile))
    assert {"ci.yml:build-test", f"{PUBLISHER}:{PUBLISHER_JOB}"} <= lanes, (
        f"the known suite lanes must be derived, got {sorted(lanes)}"
    )
    goals = nextest_goals(makefile)
    assert {"test", "test-nextest"} <= goals, (
        f"the Makefile's suite goals must be derived, got {sorted(goals)}"
    )


def test_the_publisher_cannot_drop_the_install(
    documents: Documents, makefile: str
) -> None:
    """Without it `check-build-tools` refuses the suite's `make` recipes."""
    steps = _steps(documents, PUBLISHER, PUBLISHER_JOB)
    steps.remove(_install(steps))
    _reports(documents, makefile, "never runs `make install-build-tools`")


def test_the_install_cannot_follow_the_suite(
    documents: Documents, makefile: str
) -> None:
    """An install after the coverage step leaves the suite without mold."""
    steps = _steps(documents, PUBLISHER, PUBLISHER_JOB)
    install = _install(steps)
    steps.remove(install)
    steps.append(install)
    _reports(documents, makefile, "only after")


def test_the_install_cannot_be_guarded(documents: Documents, makefile: str) -> None:
    """A guard could skip the install while the suite still runs."""
    _install(_steps(documents, PUBLISHER, PUBLISHER_JOB))["if"] = "false"
    _reports(documents, makefile, "must not carry an `if:`")


def test_the_install_cannot_continue_on_error(
    documents: Documents, makefile: str
) -> None:
    """A failed install would surface only as a test failure far downstream."""
    _install(_steps(documents, PUBLISHER, PUBLISHER_JOB))["continue-on-error"] = True
    _reports(documents, makefile, "must not continue on error")


@pytest.mark.parametrize(
    "runs_on",
    [
        "ubuntu-latest",
        ["self-hosted", "ubicloud-standard-2-ubuntu-2404"],
        {"group": "linux-runners", "labels": ["linux", "x64"]},
    ],
    ids=["scalar", "sequence", "group"],
)
@pytest.mark.parametrize(
    "command",
    ["make test", "make test-nextest", "cargo nextest run --workspace"],
)
def test_a_new_linux_suite_lane_needs_the_install(
    documents: Documents, makefile: str, runs_on: object, command: str
) -> None:
    """Every runner form and every way into the suite is held to the rule."""
    _jobs(documents, PUBLISHER)["extra"] = {
        "runs-on": runs_on,
        "steps": [{"run": command}],
    }
    _reports(documents, makefile, f"{PUBLISHER}:extra")


def test_a_windows_suite_lane_is_not_held_to_mold(
    documents: Documents, makefile: str
) -> None:
    """The pinned mold is Linux-only, so a Windows lane is outside the rule."""
    _jobs(documents, PUBLISHER)["extra"] = {
        "runs-on": "windows-latest",
        "steps": [{"run": "make test"}],
    }
    _clean(documents, makefile)


def test_a_linux_lane_without_the_suite_is_not_held(
    documents: Documents, makefile: str
) -> None:
    """A job that never runs the suite needs no mold for it."""
    _jobs(documents, PUBLISHER)["extra"] = {
        "runs-on": "ubuntu-latest",
        "steps": [{"run": "make check-fmt"}, {"run": "cargo nextest list"}],
    }
    _clean(documents, makefile)


def test_an_empty_reading_is_refused(makefile: str) -> None:
    """With no suite lane left the rule reports it rather than passing."""
    _reports({}, makefile, "no Linux workflow job runs the nextest suite")


def test_a_makefile_without_the_suite_is_refused(documents: Documents) -> None:
    """If the goal derivation reads nothing, the rule says so."""
    _reports(documents, "check-fmt:\n\tcargo fmt --check\n", "declares no goal")


def test_runner_labels_ignore_an_unresolved_shape() -> None:
    """An expression-valued runner cannot be placed, so it names no label."""
    assert not runner_labels({"runs-on": {"group": "only-a-group"}}), (
        "a group with no labels names no runner"
    )
    assert not is_linux_job({"runs-on": "${{ inputs.runner }}"}), (
        "an expression cannot be read as a Linux runner"
    )
