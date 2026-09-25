"""Verify the downstream canary runner through its command-line boundary.

Each test runs ``run_downstream_canary.py`` as a child process against fake
``netsuke`` and ``ninja`` executables and a real Git checkout standing in for
the downstream repository. The child receives an explicit environment, so no
test mutates its own process environment.
"""

import typing as typ

import downstream_canary_provenance as provenance
import pytest
from downstream_canary_test_support import (
    IDENTITY_FIELDS,
    PHASE_FIELDS,
    Canary,
    expected_identity,
    generate,
    run,
)

if typ.TYPE_CHECKING:
    from pathlib import Path


@pytest.fixture
def canary(tmp_path: Path) -> Canary:
    """Provide a fresh fake downstream checkout.

    Returns
    -------
    Canary
        The fake checkout and its working files.
    """
    return Canary(tmp_path)


def test_passing_lane_records_full_provenance(canary: Canary) -> None:
    """A clean run records every identity field and a passing outcome."""
    assert generate(canary) == 0, "generation should pass"
    assert run(canary, "lint", "test", forbid=("postgres",)) == 0, "targets pass"

    record = canary.report("lint", "test")

    identity = {key: record[key] for key in IDENTITY_FIELDS}
    assert identity == expected_identity(canary), (
        "the record should carry the run's full identity"
    )
    assert (record["generate"], record["isolation"], record["outcome"]) == (
        "passed",
        "passed",
        "passed",
    ), "every phase should pass"
    assert canary.statuses(record) == {"lint": "passed", "test": "passed"}, (
        "every target should pass"
    )
    assert set(record) == {*IDENTITY_FIELDS, *PHASE_FIELDS}, "no unexpected fields"
    assert canary.summary.read_text(encoding="utf-8").startswith(
        "- **mxd-sqlite** (sqlite) on Linux: passed."
    ), "the job summary should gain one line"


def test_selectors_reach_generation_and_every_target(canary: Canary) -> None:
    """The lane selector is in the environment of Netsuke and of Ninja."""
    generate(canary)
    run(canary, "lint")

    assert canary.log.read_text(encoding="utf-8").splitlines() == [
        "netsuke --verbose generate --output build.ninja lane=sqlite url=-",
        "ninja -f build.ninja lint lane=sqlite url=-",
    ], "both tools should see the selected lane"


def test_failed_generation_fails_and_runs_nothing(canary: Canary) -> None:
    """A generation failure fails the step and leaves every target unrun."""
    assert generate(canary, FAKE_NETSUKE_STATUS="3") == 1, "generation fails"

    record = canary.report("lint")

    assert (record["generate"], record["outcome"]) == ("failed", "failed"), (
        "the failure should be recorded"
    )
    assert canary.statuses(record) == {"lint": "not_run"}, "nothing ran"


def test_another_lanes_command_blocks_every_target(canary: Canary) -> None:
    """A manifest reaching a forbidden lane fails before any target runs."""
    generate(
        canary, FAKE_MANIFEST="build lint: phony\n  command = --features postgres\n"
    )

    assert run(canary, "lint", forbid=("postgres",)) == 1, "isolation should fail"

    record = canary.report("lint")
    assert record["isolation"] == "failed", "isolation should be recorded"
    assert canary.statuses(record) == {"lint": "not_run"}, "no target should run"
    assert not [
        line
        for line in canary.log.read_text(encoding="utf-8").splitlines()
        if line.startswith("ninja ")
    ], "Ninja should never run"


def test_a_failing_target_does_not_stop_the_rest(canary: Canary) -> None:
    """Every target runs, so the record names each one that failed."""
    generate(canary)

    assert run(canary, "check-fmt", "lint", "test", FAKE_NINJA_FAIL="lint") == 1, (
        "one failed target should fail the step"
    )

    record = canary.report("check-fmt", "lint", "test")
    assert canary.statuses(record) == {
        "check-fmt": "passed",
        "lint": "failed",
        "test": "passed",
    }, "each target should keep its own status"
    assert record["outcome"] == "failed", "the outcome should fail"


def test_an_unpinned_checkout_cannot_pass(canary: Canary) -> None:
    """A green run on a revision other than the pin is not admitted."""
    generate(canary)
    run(canary, "lint")

    record = canary.report("lint", revision="c" * 40)

    assert record["downstream_head"] == canary.head, "the observed head is recorded"
    assert record["outcome"] == "failed", "a pin mismatch should fail"


def test_report_without_state_records_a_failed_setup(canary: Canary) -> None:
    """A job that failed before generation still yields a bounded record."""
    record = canary.report("all")

    assert (record["generate"], record["isolation"], record["outcome"]) == (
        "not_run",
        "not_run",
        "failed",
    ), "an absent state should be reported as not run"


@pytest.mark.parametrize("pair", ["MXD_BACKEND", "mxd_backend=sqlite", "=sqlite"])
def test_malformed_selectors_are_refused(canary: Canary, pair: str) -> None:
    """Only ``NAME=value`` assignments with an upper-case name are accepted."""
    status = canary.step(
        "generate", "--netsuke", str(canary.netsuke), "--selector", pair
    )

    assert status != 0, f"{pair!r} should be refused"
    assert not canary.log.exists(), "Netsuke should not run"


@pytest.mark.parametrize(
    ("value", "expected"),
    [("passed", "passed"), ("failed", "failed"), ("bogus", "not_run"), (7, "not_run")],
)
def test_statuses_stay_inside_the_vocabulary(value: object, expected: str) -> None:
    """A damaged state entry is reported as ``not_run``, never verbatim."""
    assert provenance.bounded_status(value) == expected, f"status for {value!r}"
