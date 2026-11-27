"""Verify the downstream canary runner's decisions with injected inputs.

``test_run_downstream_canary.py`` drives the runner end to end against fake
executables. These tests call the pure decisions directly -- lane isolation,
the ``run`` step's exit status, the record and summary a report writes -- and
the edges with an injected :class:`Host`, so each input is chosen by the test
rather than read from this process's environment or ``PATH``.
"""

import argparse
import json
import typing as typ

import pytest
import run_downstream_canary as runner
from downstream_canary_isolation import Isolation, judge_isolation, run_status
from downstream_canary_process import Host
from downstream_canary_provenance import CanaryIdentity
from downstream_canary_state import (
    StateUnreadableError,
    read_state,
    state_or_nothing_run,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

REVISION = "c" * 40
PASSING_STATE = {
    "generate": "passed",
    "isolation": "passed",
    "targets": {"lint": "passed"},
}


class RecordingWhich:
    """Answer executable lookups from a table and record every question."""

    def __init__(self, answers: dict[str, str | None]) -> None:
        """Hold the answer for each executable name."""
        self.answers = answers
        self.calls: list[tuple[str, str | None]] = []

    def __call__(self, name: str, /, *, path: str | None = None) -> str | None:
        """Record the lookup and return the table's answer."""
        self.calls.append((name, path))
        return self.answers.get(name)


def _identity() -> CanaryIdentity:
    """Return a fixed run identity pinned to ``REVISION``."""
    return CanaryIdentity(
        canary="mxd-sqlite",
        repository="leynos/mxd",
        downstream_revision=REVISION,
        netsuke_revision="b" * 40,
        netsuke_version="0.1.0-rc1",
        platform="Linux",
        selectors={"MXD_BACKEND": "sqlite"},
        targets=("lint",),
    )


@pytest.mark.parametrize(
    ("manifest", "patterns", "found", "status"),
    [
        pytest.param("build lint: phony\n", ["postgres"], (), "passed", id="clean"),
        pytest.param(
            "command = --features postgres wireframe\n",
            ["wireframe", "sqlite", "postgres"],
            ("wireframe", "postgres"),
            "failed",
            id="two-lanes",
        ),
        pytest.param("anything\n", [], (), "passed", id="no-patterns"),
    ],
)
def test_isolation_names_every_forbidden_pattern(
    manifest: str, patterns: list[str], found: tuple[str, ...], status: str
) -> None:
    """The verdict lists each pattern found, in the order the lane gave them."""
    verdict = judge_isolation(manifest, patterns)

    assert verdict.found == found, f"found patterns for {patterns!r}"
    assert verdict.status == status, f"isolation status for {patterns!r}"


@pytest.mark.parametrize(
    ("isolation", "statuses", "expected"),
    [
        pytest.param(Isolation(()), {"lint": "passed", "test": "passed"}, 0, id="ok"),
        pytest.param(Isolation(()), {"lint": "passed", "test": "failed"}, 1, id="fail"),
        pytest.param(Isolation(("pg",)), {"lint": "not_run"}, 1, id="isolation"),
        pytest.param(Isolation(()), {}, 0, id="no-targets"),
    ],
)
def test_run_status_fails_on_isolation_or_any_failed_target(
    isolation: Isolation, statuses: dict[str, str], expected: int
) -> None:
    """``run`` exits non-zero when isolation fails or any target failed."""
    assert run_status(isolation, statuses) == expected, f"status for {statuses!r}"


@pytest.mark.parametrize(
    ("state", "head", "outcome"),
    [
        pytest.param(PASSING_STATE, REVISION, "passed", id="pinned-and-passing"),
        pytest.param(PASSING_STATE, "d" * 40, "failed", id="unpinned-head"),
        pytest.param(PASSING_STATE, None, "failed", id="unknown-head"),
        pytest.param({}, REVISION, "failed", id="nothing-recorded"),
    ],
)
def test_compose_report_derives_the_outcome_from_its_inputs(
    state: dict[str, object], head: str | None, outcome: str
) -> None:
    """The record passes only for a passing state on the pinned revision."""
    composed = runner.compose_report(_identity(), state, head)

    assert composed.record["outcome"] == outcome, f"outcome for head {head!r}"
    assert composed.record["downstream_head"] == head, "the head is recorded"
    assert composed.summary.startswith(
        f"- **mxd-sqlite** (sqlite) on Linux: {outcome}."
    ), f"the summary must report {outcome}: {composed.summary!r}"


def test_child_environment_reads_only_the_injected_environment() -> None:
    """Assignments layer over the injected environment, selectors last."""
    arguments = argparse.Namespace(
        environment=["PATH=/child", "MXD_BACKEND=postgres"],
        selector=["MXD_BACKEND=sqlite"],
    )

    env = runner.child_environment(arguments, {"PATH": "/step", "KEEP": "1"})

    assert env == {"PATH": "/child", "KEEP": "1", "MXD_BACKEND": "sqlite"}, (
        f"only the injected environment and assignments may appear: {env!r}"
    )


def test_ninja_is_looked_up_on_the_child_path() -> None:
    """The lookup is asked for Ninja on the child environment's ``PATH``."""
    which = RecordingWhich({"ninja": "/child/ninja"})

    assert runner.resolve_ninja({"PATH": "/child"}, which) == "/child/ninja", (
        "the injected lookup's answer must be used"
    )
    assert which.calls == [("ninja", "/child")], f"lookups: {which.calls!r}"


@pytest.mark.parametrize(
    "content",
    [
        pytest.param(None, id="missing"),
        pytest.param("{not json", id="not-json"),
        pytest.param("[1, 2]", id="not-an-object"),
    ],
)
def test_read_state_raises_for_an_unusable_file(
    tmp_path: Path, content: str | None
) -> None:
    """The loader reports an unusable state instead of inventing an empty one."""
    path = tmp_path / "state.json"
    if content is not None:
        path.write_text(content, encoding="utf-8")

    with pytest.raises(StateUnreadableError):
        read_state(path)


def test_an_unusable_state_is_reported_and_read_as_nothing_run(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """The edges' decision is explicit: an empty state, and a stated reason."""
    state = state_or_nothing_run(tmp_path / "absent.json")

    assert state == {}, "an unusable state must read as nothing recorded"
    assert "recording unreached phases as not_run" in capsys.readouterr().err, (
        "the reason must reach the job log"
    )


def test_report_asks_the_injected_host_for_git(tmp_path: Path) -> None:
    """``report`` finds Git through the injected host, not the ambient ``PATH``.

    The lookup answers ``None``, so no Git runs and the head is unknown; the
    record is still written, and fails because the head is unproven.
    """
    which = RecordingWhich({"git": None})
    provenance = tmp_path / "provenance.json"
    (tmp_path / "state.json").write_text(json.dumps(PASSING_STATE), encoding="utf-8")
    host = Host({"PATH": "/injected"}, which)

    status = runner.main(
        [
            "report",
            *("--workdir", str(tmp_path), "--state", str(tmp_path / "state.json")),
            *("--target", "lint", "--selector", "MXD_BACKEND=sqlite"),
            *("--canary", "mxd-sqlite", "--repository", "leynos/mxd"),
            *("--downstream-revision", REVISION, "--netsuke-revision", "b" * 40),
            *("--netsuke-version", "0.1.0-rc1", "--platform", "Linux"),
            *("--provenance", str(provenance)),
        ],
        host=host,
    )

    assert status == 0, "reporting must never fail the step"
    assert which.calls == [("git", "/injected")], f"lookups: {which.calls!r}"
    record = json.loads(provenance.read_text(encoding="utf-8"))
    assert (record["downstream_head"], record["outcome"]) == (None, "failed"), (
        "an unobservable head must fail the record"
    )
