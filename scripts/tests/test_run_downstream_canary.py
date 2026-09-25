"""Verify the downstream canary runner through its command-line boundary.

Each test runs ``run_downstream_canary.py`` as a child process against fake
``netsuke`` and ``ninja`` executables and a real Git checkout standing in for
the downstream repository. The child receives an explicit environment, so no
test mutates its own process environment.
"""

import json
import os
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the runner's process boundary is under test.
import sys
from pathlib import Path

import downstream_canary_provenance as provenance
import pytest

SCRIPTS = Path(__file__).resolve().parents[1]
RUNNER = SCRIPTS / "run_downstream_canary.py"

#: Records its arguments and lane, writes the manifest, and exits as asked.
FAKE_NETSUKE = """\
import os, pathlib, sys
default = "build ok: phony\\n"
pathlib.Path("build.ninja").write_text(os.environ.get("FAKE_MANIFEST", default))
lane = os.environ.get("MXD_BACKEND", "-")
with pathlib.Path(os.environ["FAKE_LOG"]).open("a") as log:
    log.write(f"netsuke {' '.join(sys.argv[1:])} lane={lane}\\n")
sys.exit(int(os.environ.get("FAKE_NETSUKE_STATUS", "0")))
"""
#: Records its arguments and lane, and fails the targets named in the fixture.
FAKE_NINJA = """\
import os, pathlib, sys
lane = os.environ.get("MXD_BACKEND", "-")
with pathlib.Path(os.environ["FAKE_LOG"]).open("a") as log:
    log.write(f"ninja {' '.join(sys.argv[1:])} lane={lane}\\n")
failing = os.environ.get("FAKE_NINJA_FAIL", "").split(",")
sys.exit(1 if sys.argv[-1] in failing else 0)
"""


#: The record fields that identify the run for release review.
IDENTITY_FIELDS = (
    "schema",
    "canary",
    "repository",
    "downstream_revision",
    "downstream_head",
    "netsuke_revision",
    "netsuke_version",
    "platform",
    "selectors",
)
#: The record fields that report the run's result.
PHASE_FIELDS = ("generate", "isolation", "targets", "outcome")


class Canary:
    """Hold one fake downstream checkout and the runner's working files."""

    def __init__(self, root: Path) -> None:
        """Create the fake tools and a committed downstream checkout."""
        self.root = root
        self.bin = root / "bin"
        self.workdir = root / "downstream"
        self.state = root / "state.json"
        self.log = root / "tools.log"
        self.provenance = root / "provenance.json"
        self.summary = root / "summary.md"
        self.bin.mkdir()
        self.workdir.mkdir()
        self.netsuke = self.write_tool("netsuke", FAKE_NETSUKE)
        self.write_tool("ninja", FAKE_NINJA)
        self.git("init", "--quiet")
        self.git("commit", "--quiet", "--allow-empty", "-m", "downstream")
        self.head = self.git("rev-parse", "HEAD")

    def write_tool(self, name: str, source: str) -> Path:
        """Write an executable Python tool into the fake ``bin`` directory.

        Returns
        -------
        Path
            The tool's path.
        """
        path = self.bin / name
        path.write_text(f"#!{sys.executable}\n{source}", encoding="utf-8")
        path.chmod(0o755)
        return path

    def environment(self, **extra: str) -> dict[str, str]:
        """Return an explicit child environment with the fake tools first.

        Returns
        -------
        dict[str, str]
            The environment for one runner invocation.
        """
        return {
            "PATH": f"{self.bin}{os.pathsep}{os.environ.get('PATH', '')}",
            "FAKE_LOG": str(self.log),
            "GIT_CONFIG_GLOBAL": os.devnull,
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_AUTHOR_NAME": "Canary",
            "GIT_AUTHOR_EMAIL": "canary@example.invalid",
            "GIT_COMMITTER_NAME": "Canary",
            "GIT_COMMITTER_EMAIL": "canary@example.invalid",
            **extra,
        }

    def git(self, *arguments: str) -> str:
        """Run Git in the downstream checkout.

        Returns
        -------
        str
            Git's standard output, stripped.
        """
        return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed program, list arguments.
            ["git", "-C", str(self.workdir), *arguments],  # ruff: ignore[start-process-with-partial-path] - Git on PATH.
            check=True,
            capture_output=True,
            text=True,
            env=self.environment(),
        ).stdout.strip()

    def step(self, command: str, *arguments: str, **extra: str) -> int:
        """Run one runner subcommand and return its exit status.

        Returns
        -------
        int
            The runner's exit status.
        """
        common = ["--workdir", str(self.workdir), "--state", str(self.state)]
        return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed interpreter and script.
            [sys.executable, str(RUNNER), command, *common, *arguments],
            check=False,
            env=self.environment(**extra),
        ).returncode

    def report(self, *targets: str, revision: str | None = None) -> dict:
        """Run ``report`` for ``targets`` and return the provenance record.

        Returns
        -------
        dict
            The parsed provenance record.
        """
        target_arguments = [part for target in targets for part in ("--target", target)]
        status = self.step(
            "report",
            *target_arguments,
            "--selector",
            "MXD_BACKEND=sqlite",
            "--canary",
            "mxd-sqlite",
            "--repository",
            "leynos/mxd",
            "--downstream-revision",
            revision or self.head,
            "--netsuke-revision",
            "b" * 40,
            "--netsuke-version",
            "0.1.0-rc1",
            "--platform",
            "Linux",
            "--provenance",
            str(self.provenance),
            "--summary",
            str(self.summary),
        )
        assert status == 0, "reporting must never fail the step"
        return json.loads(self.provenance.read_text(encoding="utf-8"))

    def statuses(self, record: dict) -> dict[str, str]:
        """Return the record's per-target statuses.

        Returns
        -------
        dict[str, str]
            Target name to status.
        """
        return {target["name"]: target["status"] for target in record["targets"]}


def expected_identity(canary: Canary) -> dict[str, object]:
    """Return the identity fields a SQLite-lane report should record.

    Returns
    -------
    dict[str, object]
        The expected identity, keyed by :data:`IDENTITY_FIELDS`.
    """
    return {
        "schema": provenance.SCHEMA,
        "canary": "mxd-sqlite",
        "repository": "leynos/mxd",
        "downstream_revision": canary.head,
        "downstream_head": canary.head,
        "netsuke_revision": "b" * 40,
        "netsuke_version": "0.1.0-rc1",
        "platform": "Linux",
        "selectors": {"MXD_BACKEND": "sqlite"},
    }


@pytest.fixture
def canary(tmp_path: Path) -> Canary:
    """Provide a fresh fake downstream checkout.

    Returns
    -------
    Canary
        The fake checkout and its working files.
    """
    return Canary(tmp_path)


def generate(canary: Canary, **extra: str) -> int:
    """Run the ``generate`` step with the SQLite lane selected.

    Returns
    -------
    int
        The step's exit status.
    """
    return canary.step(
        "generate",
        "--netsuke",
        str(canary.netsuke),
        "--selector",
        "MXD_BACKEND=sqlite",
        **extra,
    )


def run(
    canary: Canary, *targets: str, forbid: tuple[str, ...] = (), **extra: str
) -> int:
    """Run the ``run`` step for ``targets`` with the SQLite lane selected.

    Returns
    -------
    int
        The step's exit status.
    """
    arguments = [part for target in targets for part in ("--target", target)]
    arguments += [part for pattern in forbid for part in ("--forbid", pattern)]
    return canary.step("run", *arguments, "--selector", "MXD_BACKEND=sqlite", **extra)


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
        "netsuke --verbose generate --output build.ninja lane=sqlite",
        "ninja -f build.ninja lint lane=sqlite",
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
