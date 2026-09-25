"""Provide the fake downstream checkout that drives the canary runner.

The runner is exercised through its command line against fake ``netsuke`` and
``ninja`` executables and a real Git checkout. Every invocation receives an
explicit child environment, so no test mutates its own process environment.
"""

import json
import os
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the runner's process boundary is under test.
import sys
from pathlib import Path

import downstream_canary_provenance as provenance

SCRIPTS = Path(__file__).resolve().parents[1]
RUNNER = SCRIPTS / "run_downstream_canary.py"

#: Records its arguments and lane, writes the manifest, and exits as asked.
FAKE_NETSUKE = """\
import os, pathlib, sys
default = "build ok: phony\\n"
pathlib.Path("build.ninja").write_text(os.environ.get("FAKE_MANIFEST", default))
lane = os.environ.get("MXD_BACKEND", "-")
with pathlib.Path(os.environ["FAKE_LOG"]).open("a") as log:
    url = os.environ.get("POSTGRES_TEST_URL", "-")
    log.write(f"netsuke {' '.join(sys.argv[1:])} lane={lane} url={url}\\n")
sys.exit(int(os.environ.get("FAKE_NETSUKE_STATUS", "0")))
"""
#: Records its arguments and lane, and fails the targets named in the fixture.
FAKE_NINJA = """\
import os, pathlib, sys
lane = os.environ.get("MXD_BACKEND", "-")
with pathlib.Path(os.environ["FAKE_LOG"]).open("a") as log:
    url = os.environ.get("POSTGRES_TEST_URL", "-")
    log.write(f"ninja {' '.join(sys.argv[1:])} lane={lane} url={url}\\n")
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
        return self.completed_step(command, *arguments, **extra).returncode

    def completed_step(
        self, command: str, *arguments: str, **extra: str
    ) -> subprocess.CompletedProcess[str]:
        """Run one runner subcommand, capturing its output.

        Returns
        -------
        subprocess.CompletedProcess[str]
            The finished runner process.
        """
        common = ["--workdir", str(self.workdir), "--state", str(self.state)]
        return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed interpreter and script.
            [sys.executable, str(RUNNER), command, *common, *arguments],
            check=False,
            capture_output=True,
            text=True,
            env=self.environment(**extra),
        )

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
