"""Run each release producer's own provenance step and collect what it wrote.

The action's execution cases start from a tree somebody else assembled, so the
step that actually *binds* bytes to a candidate -- the producer's "Record
producer provenance" step -- was never executed. That left the hand-off
untested at the seam that matters: a record written into the wrong directory,
or covering a package set the release never declares, would satisfy every
structural check and still publish bytes nobody vouched for.

This module runs that step's real body, read from ``build-and-package.yml``
rather than restated, once per configured target. Each run gets the tree its
own producer really holds, named and laid out by
``upload_declared_assets_producer_tree.py``, with ``uv`` stubbed at the path
boundary for the same reason the action's harness stubs it: the subject is the
step's own script, not an interpreter download.

:func:`assemble_publication` then replays what sealing does with those trees:
each run's ``dist`` is nested under one directory of its own, as
``download-artifact`` nests one artifact per producer, and the archives are
hoisted to the root by the release's own hoist function. What survives is the
tree the publication action consumes, built only from bytes the producers
wrote.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import json
import shutil
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the producer step is under test.
import sys
import typing as typ

import hoist_binstall_archives
import upload_declared_assets_producer_env as producer_env
import upload_declared_assets_producer_tree as tree
import upload_declared_assets_staging as staging
from upload_declared_assets_staging import Facts, Target
from upload_declared_assets_stubs import UV_STUB

if typ.TYPE_CHECKING:
    import pathlib


@dataclasses.dataclass(frozen=True, slots=True)
class ProducerRun:
    """Report what executing one producer's recording step did.

    Attributes
    ----------
    target
        The configured target the producer served.
    workspace
        The producer's own tree, holding its ``dist`` as the step left it.
    completed
        The completed step.
    record_path
        Where the producer wrote its record, inside ``workspace``.
    record
        The decoded record, or an empty mapping when it wrote none.
    baselines
        Every interpreter the stubbed ``uv`` was told to use.
    """

    target: Target
    workspace: pathlib.Path
    completed: subprocess.CompletedProcess[str]
    record_path: pathlib.Path
    record: dict[str, object]
    baselines: tuple[str, ...]

    @property
    def record_dir(self) -> pathlib.Path:
        """The directory the record sits in, relative to the producer's tree."""
        return self.record_path.parent.relative_to(self.workspace)

    @property
    def claimed(self) -> dict[str, str]:
        """The asset names the producer recorded, with their digests."""
        assets = self.record.get("assets")
        if not isinstance(assets, dict):
            return {}
        return {str(name): str(digest) for name, digest in assets.items()}


@dataclasses.dataclass(frozen=True, slots=True)
class Publication:
    """Hold the tree the publication action consumes, and how it was sealed.

    Attributes
    ----------
    workspace
        The workspace the action's steps run in.
    hoisted
        What the release's own hoist function returned.
    runs
        The producer runs whose bytes the tree is built from.
    """

    workspace: pathlib.Path
    hoisted: int
    runs: tuple[ProducerRun, ...]


def run_producer_step(
    workspace: pathlib.Path, environment: dict[str, str], *, label: str
) -> subprocess.CompletedProcess[str]:
    """Run the producer step's real body once, over an already-staged tree.

    The body is taken from the workflow rather than restated, so its ``case``
    arms, its ``nullglob`` setting, and its recorder argument list are each
    held to account as written.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The completed step.
    """
    stubs = workspace / "bin"
    stubs.mkdir(exist_ok=True)
    (stubs / "uv").write_text(UV_STUB, encoding="utf-8")
    (stubs / "uv").chmod(0o755)
    script = workspace / f"record-{label}.sh"
    body = str(producer_env.producer_step().get("run", ""))
    script.write_text(body, encoding="utf-8")
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the step body is the shell script.
        ["bash", str(script)],  # ruff: ignore[start-process-with-partial-path] - resolved from the fixed PATH.
        cwd=workspace,
        check=False,
        env={
            "PATH": f"{stubs}:/usr/bin:/bin",
            "HOME": str(workspace.parent),
            "TMPDIR": str(workspace.parent),
            "PYTHONDONTWRITEBYTECODE": "1",
            "UV_PYTHON": sys.executable,
        }
        | environment,
        text=True,
        capture_output=True,
    )


def run_producer(tmp_path: pathlib.Path, facts: Facts, target: Target) -> ProducerRun:
    """Execute one target's producer recording step over its own tree.

    Returns
    -------
    ProducerRun
        The completed step, its record, and the tree it left behind.
    """
    workspace = staging.stage_repository(tmp_path, name=f"producer-{target.key}")
    tree.stage_producer_tree(workspace, facts, target)
    baseline_log = tmp_path / f"uv-{target.key}.txt"
    baseline_log.touch()
    completed = run_producer_step(
        workspace,
        producer_env.step_environment(facts, target)
        | {"UV_BASELINE_LOG": str(baseline_log)},
        label=target.key,
    )
    record_path = workspace / tree.record_location(target, facts) / staging.RECORD_NAME
    return ProducerRun(
        target=target,
        workspace=workspace,
        completed=completed,
        record_path=record_path,
        record=decode_record(record_path),
        baselines=tuple(baseline_log.read_text(encoding="utf-8").split()),
    )


def decode_record(path: pathlib.Path) -> dict[str, object]:
    """Return the record at ``path``, or an empty mapping when there is none.

    Returns
    -------
    dict[str, object]
        The decoded record.
    """
    if not path.is_file():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def assemble_publication(
    tmp_path: pathlib.Path, facts: Facts, runs: tuple[ProducerRun, ...]
) -> Publication:
    """Seal the producers' trees into the tree the publication action reads.

    The nesting reproduces what ``download-artifact`` does with one artifact
    per producer, and the hoist is the release's own function rather than a
    copy of it, so the placement the action depends on is the placement the
    release performs.

    Returns
    -------
    Publication
        The workspace the action runs in, and the hoist's exit status.
    """
    workspace = staging.stage_repository(tmp_path, name="publication")
    dist = workspace / staging.DIST
    dist.mkdir()
    for run in runs:
        shutil.copytree(
            run.workspace / staging.DIST, dist / run.target.key, dirs_exist_ok=True
        )
    hoisted = hoist_binstall_archives.hoist(
        dist,
        workspace / ".github" / "release-staging.toml",
        workspace / "Cargo.toml",
        facts.version,
    )
    return Publication(workspace=workspace, hoisted=hoisted, runs=runs)
