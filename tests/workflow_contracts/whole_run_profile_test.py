"""Every coverage lane runs under the profile that carries the budget.

The whole-run budget lives in one nextest profile rather than in
``default``, so it reaches a run only if that run selects it. Nothing
else in the tree would say so: ``whole_run_value_test`` reads the
budget straight out of the file and passes whether or not any lane ever
asks for it, and the ordering assertions read it the same way. A lane
that stopped selecting the profile would run uncapped while every other
assertion about the budget still held.

The shared ``generate-coverage`` action takes no profile input, so the
``NEXTEST_PROFILE`` environment variable is the only lever, and this
contract asserts the variable each lane actually resolves rather than
its presence anywhere in the file.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from coverage_lanes import CoverageLane, coverage_lanes_of
from lane_environment import nextest_profile_of
from nextest_budgets import global_timeout
from timeout_budgets import (
    CAPPED_PROFILE,
    NEXTEST_CONFIG,
    NEXTEST_PROFILE_VARIABLE,
)


@pytest.fixture(scope="module")
def coverage_lanes() -> tuple[CoverageLane, ...]:
    """Return every step invoking the coverage action.

    Returns
    -------
    tuple[CoverageLane, ...]
        One entry per coverage step.
    """
    return coverage_lanes_of()


def test_every_coverage_lane_selects_the_capped_profile(
    coverage_lanes: tuple[CoverageLane, ...],
) -> None:
    """The budget reaches a lane only through the profile it selects.

    A lane naming no profile runs under ``default``, which carries no
    whole-run budget by design, so the omission is silent: the run
    simply has no tier two and the watchdog resumes doing its job. A
    lane naming some other profile is the same fault wearing a value.

    Proved by mutation: deleting ``NEXTEST_PROFILE`` from either
    coverage job, and setting it to ``default`` in either, each fail
    this test.
    """
    assert coverage_lanes, (
        "no coverage lane was found, so this contract would pass over an "
        "empty list; the action moved or the lane reading stopped matching it"
    )
    wrong = {
        str(lane): lane.nextest_profile
        for lane in coverage_lanes
        if lane.nextest_profile != CAPPED_PROFILE
    }
    assert not wrong, (
        f"these coverage lanes do not set {NEXTEST_PROFILE_VARIABLE} to "
        f"{CAPPED_PROFILE!r}, so the whole-run budget does not apply to them "
        f"and the cargo watchdog is doing tier two's job again, as found "
        f"versus required {CAPPED_PROFILE!r}: {wrong}; None means the lane "
        f"names no profile and so runs under default, which carries no "
        f"whole-run budget"
    )


def test_the_selected_profile_is_the_one_carrying_the_budget() -> None:
    """The variable and the configuration have to name the same profile.

    The two halves of this change live in different files and nothing
    makes them agree. Renaming the profile in `.config/nextest.toml`
    while the workflows still select the old name leaves a budget
    nothing reads and lanes selecting a profile that does not exist,
    which nextest accepts by falling back to ``default``.
    """
    config_text = NEXTEST_CONFIG.read_text(encoding="utf-8")

    assert global_timeout(config_text, CAPPED_PROFILE) is not None, (
        f"the coverage lanes select [profile.{CAPPED_PROFILE}] but that "
        f"profile declares no global-timeout, so every lane runs with no "
        f"whole-run budget while the configuration appears to set one"
    )
    assert global_timeout(config_text, "default") is None, (
        "the whole-run budget is in [profile.default], where every local "
        "`make test` reads it. It belongs to the profile CI selects: a "
        "developer's host is contended in a way CI's is not, and a cap sized "
        "from CI logs would end a local run against a figure that says "
        "nothing about it"
    )


@pytest.mark.parametrize(
    ("document", "job", "step", "expected"),
    [
        pytest.param(
            {"env": {NEXTEST_PROFILE_VARIABLE: "workflow"}},
            {"env": {NEXTEST_PROFILE_VARIABLE: "job"}},
            {"env": {NEXTEST_PROFILE_VARIABLE: "step"}},
            "step",
            id="the-step-wins",
        ),
        pytest.param(
            {"env": {NEXTEST_PROFILE_VARIABLE: "workflow"}},
            {"env": {NEXTEST_PROFILE_VARIABLE: "job"}},
            {},
            "job",
            id="then-the-job",
        ),
        pytest.param(
            {"env": {NEXTEST_PROFILE_VARIABLE: "workflow"}},
            {},
            {},
            "workflow",
            id="then-the-workflow",
        ),
        pytest.param({}, {}, {}, None, id="nowhere-reads-as-absent"),
        pytest.param(
            {},
            {"env": {NEXTEST_PROFILE_VARIABLE: "   "}},
            {},
            None,
            id="blank-reads-as-absent",
        ),
    ],
)
def test_the_profile_is_resolved_from_every_environment_scope(
    document: dict[str, typ.Any],
    job: dict[str, typ.Any],
    step: dict[str, typ.Any],
    expected: str | None,
) -> None:
    """Step, then job, then workflow, as GitHub resolves them.

    Both lanes here set the variable at job level, so against the real
    tree a reading that consulted only the job would agree with this
    one. The scopes it never reaches are what these cases execute, and a
    blank value is included because that is what an interpolated
    expression writes when it resolves to nothing: read as a profile
    name it would be a profile that cannot exist.
    """
    assert nextest_profile_of(document, job, step) == expected, (
        f"the profile must resolve to {expected!r} from these scopes; a "
        f"reading consulting fewer of them reports a lane as running under "
        f"default while an outer scope selected a profile"
    )
