"""Execute the declared-asset action's own shell steps outside GitHub.

The suites around ``.github/actions/upload-declared-assets`` read its manifest
as text: they assert which script each step names, that no ``run`` block
carries a GitHub expression, and that the declaration, digest, and
reconciliation helpers each behave in isolation. None of them runs the steps,
so a step that spells a flag wrongly, passes a path its CLI cannot open, or
writes its outputs under a name its caller does not read can satisfy every
structural check and still fail on a runner.

This module closes that gap. Each ``run`` body is taken from the manifest (read
by ``upload_declared_assets_manifest.py``) rather than restated here, so the
flag spelling, the argument order, and the deferred ``upload-error`` contract
are held to account as written. ``uv`` and ``gh`` are stubbed at the path
boundary, so the real CLIs execute without downloading an interpreter or
contacting GitHub; everything else -- the staged tree, the producer records,
the payload, and the trusted digest record -- is written and read for real.

The staged tree is built by ``upload_declared_assets_staging.py``; the cases
that use this harness live in ``upload_declared_assets_execution_test.py``.

Run via ``make test-workflow-contracts``.
"""

import collections.abc as cabc
import dataclasses
import json
import sys
import typing as typ

from composite_action_execution import (
    output_file,
    read_step_outputs,
    run_shell,
    write_stub,
)
from upload_declared_assets_manifest import (
    UPLOAD_STEP,
    manifest_step_id,
    run_bodies,
    step_environment,
)
from upload_declared_assets_stubs import GH_HELPER, GH_STUB, UV_STUB

if typ.TYPE_CHECKING:
    import subprocess
    from pathlib import Path

    from upload_declared_assets_staging import Facts


@dataclasses.dataclass(frozen=True, slots=True)
class ActionRun:
    """Report what executing the action's steps did.

    Attributes
    ----------
    completed
        Each step's completed process, in execution order. Fewer than
        :data:`RUN_STEPS` when a step failed and the job stopped there.
    step_outputs
        The ``GITHUB_OUTPUT`` entries each step wrote, keyed by the step's
        manifest id.
    declared_root
        The directory the action was told to keep its records in.
    gh_log
        Every recorded ``gh`` invocation, one per line.
    baselines
        The interpreter each ``uv`` invocation was told to use.
    """

    completed: tuple[subprocess.CompletedProcess[str], ...]
    step_outputs: dict[str, dict[str, str]]
    declared_root: Path
    gh_log: str
    baselines: tuple[str, ...]

    @property
    def payload(self) -> Path:
        """The directory the selection step installed the payload in."""
        return self.declared_root / "selected" / "payload"

    @property
    def outputs(self) -> dict[str, str]:
        """The ``GITHUB_OUTPUT`` entries the upload step wrote.

        Kept as a named accessor over :attr:`step_outputs` because almost every
        case reads that step's pair and only the hand-off case has to look at
        another step's file.
        """
        return self.step_outputs[manifest_step_id(UPLOAD_STEP)]


@dataclasses.dataclass(frozen=True, slots=True)
class Sandbox:
    """Hold the paths one executed run writes and reads.

    Attributes
    ----------
    stubs
        The directory holding the ``uv`` and ``gh`` stand-ins.
    scratch
        A writable home for the child processes, also holding their scripts.
    outputs_directory
        The directory holding one ``GITHUB_OUTPUT`` file per script step, so a
        step's entries can be read back as that step's alone.
    gh_log
        Every ``gh`` invocation the stub recorded.
    attached
        The release listing the ``gh`` stub serves.
    helper
        The Python program the ``gh`` stub runs to read and update the listing.
    baseline_log
        Every interpreter the ``uv`` stub was told to use.
    declared_root
        The directory the action keeps its records in.
    """

    stubs: Path
    scratch: Path
    outputs_directory: Path
    gh_log: Path
    attached: Path
    helper: Path
    baseline_log: Path
    declared_root: Path


#: What a case may do to the run's records between two of its steps.
#:
#: The steps write their records to one directory, so a case that wants to
#: damage what a later step reads has to act in the gap between them. It is
#: handed the next step's index and the directory the records live in, and it
#: returns nothing: the run continues exactly as it would have.
type BeforeStep = cabc.Callable[[int, Path], None]


@dataclasses.dataclass(frozen=True, slots=True)
class Release:
    """Describe everything a run is handed from outside its own tree.

    Attributes
    ----------
    attached
        The assets the release already holds, as the listing reports them.
    dry_run
        The value the caller sets the ``dry-run`` input to.
    before_step
        Called with the next step's index and the records directory before
        that step runs. A case that damages a record one step wrote uses this
        rather than reaching into the workspace after the whole run, which
        would only ever model damage no later step reads.

    Notes
    -----
    These are one value because each describes the *outside* a run is handed
    rather than the tree it works on: a case that models a partially attached
    release in a rehearsal asks for its listing and its mode at once, and
    either alone would leave the other at a default the case did not choose.
    The hook belongs here for the same reason -- it stands in for a second
    attempt or a damaged runner, neither of which the tree can express.
    """

    attached: tuple[dict[str, str], ...] = ()
    dry_run: str = "false"
    before_step: BeforeStep | None = None


#: The release a case that names none is handed: nothing attached, publishing.
NO_RELEASE = Release()


def _sandbox(tmp_path: Path, attached: list[dict[str, str]] | None) -> Sandbox:
    """Create the stubs and logs one run needs.

    Returns
    -------
    Sandbox
        The paths the run writes.
    """
    # Each call gets a directory of its own. A case that proves a refusal after
    # sealing a good tree runs the action twice in one workspace, and a shared
    # log or release listing would carry the first run's uploads into the
    # second run's assertions.
    runs = tmp_path / "runs"
    runs.mkdir(exist_ok=True)
    root = runs / f"run-{len(tuple(runs.iterdir()))}"
    root.mkdir()
    stubs = root / "stubs"
    stubs.mkdir()
    write_stub(stubs / "uv", UV_STUB)
    write_stub(stubs / "gh", GH_STUB)
    # The helper lives beside the stubs but is run, not exec'd from PATH, so it
    # needs no execute bit and cannot be picked up as a command by accident.
    helper = stubs / "gh_state.py"
    helper.write_text(GH_HELPER, encoding="utf-8")
    scratch = root / "scratch"
    scratch.mkdir()
    baseline_log = root / "uv-baselines.txt"
    baseline_log.touch()
    gh_log = root / "gh.log"
    gh_log.touch()
    listing = root / "gh-attached.json"
    listing.write_text(json.dumps({"assets": attached or []}), encoding="utf-8")
    outputs_directory = root / "github-output"
    outputs_directory.mkdir()
    return Sandbox(
        stubs=stubs,
        scratch=scratch,
        outputs_directory=outputs_directory,
        gh_log=gh_log,
        attached=listing,
        helper=helper,
        baseline_log=baseline_log,
        declared_root=root / "runner" / "declared-assets",
    )


def _environment(sandbox: Sandbox) -> dict[str, str]:
    """Return the environment every step inherits.

    The stubs come first on ``PATH`` so the steps reach them rather than any
    real install, and ``HOME`` and ``TMPDIR`` point at scratch so nothing
    writes into the reader's own caches. ``GITHUB_OUTPUT`` is deliberately
    absent: a step's output file is that step's own, and :func:`run_steps`
    supplies it per step.

    Returns
    -------
    dict[str, str]
        The base environment.
    """
    return {
        "PATH": f"{sandbox.stubs}:/usr/bin:/bin",
        "HOME": str(sandbox.scratch),
        "TMPDIR": str(sandbox.scratch),
        "PYTHONDONTWRITEBYTECODE": "1",
        "GITHUB_RUN_ID": "4242",
        "GH_LOG": str(sandbox.gh_log),
        "GH_ATTACHED": str(sandbox.attached),
        "GH_HELPER": str(sandbox.helper),
        "GH_PYTHON": sys.executable,
        "UV_BASELINE_LOG": str(sandbox.baseline_log),
        "UV_PYTHON": sys.executable,
    }


@dataclasses.dataclass(frozen=True, slots=True)
class RunSetup:
    """Hold everything one executed run needs beyond its tree and its facts.

    Attributes
    ----------
    sandbox
        The paths the run writes to and reads from.
    dry_run
        The value the caller sets the ``dry-run`` input to. It reaches the
        upload step as an environment variable, which is what its ``case``
        statement reads.
    before_step
        Called with the next step's index and the records directory before
        that step runs.

    Notes
    -----
    ``execute`` takes the caller's :class:`Release` and resolves it into this
    value, so the three things a run is handed from outside its tree travel
    together rather than as a growing keyword list.
    """

    sandbox: Sandbox
    dry_run: str
    before_step: BeforeStep | None = None


def run_steps(
    workspace: Path,
    facts: Facts,
    run: RunSetup,
) -> list[subprocess.CompletedProcess[str]]:
    """Run each script step in order, stopping where a job would stop.

    Parameters
    ----------
    run
        The sandbox to write into, the mode to run in, and the hook a case
        may damage a record from.

    A failed step stops the loop because GitHub would not start the next one;
    the caller can then tell a refusal from a later upload fault by how many
    steps completed.

    Each step is handed a ``GITHUB_OUTPUT`` file of its own, exactly as a
    runner does. Sharing one file would make every step's entries
    indistinguishable from every other's, so a case could resolve the action's
    ``outputs:`` expressions against the wrong step and still find the value it
    expected.

    Returns
    -------
    list[subprocess.CompletedProcess[str]]
        Each completed step, in execution order.
    """
    sandbox = run.sandbox
    steps = sandbox.scratch / "steps"
    steps.mkdir()
    base_env = _environment(sandbox)
    completed: list[subprocess.CompletedProcess[str]] = []
    for number, step in enumerate(run_bodies()):
        if run.before_step is not None:
            run.before_step(number, sandbox.declared_root)
        script = steps / f"{number}.sh"
        script.write_text(str(step.get("run", "")), encoding="utf-8")
        step_id = manifest_step_id(str(step.get("name", "")))
        environment = base_env | step_environment(
            step,
            facts=facts,
            declared_root=sandbox.declared_root,
            dry_run=run.dry_run,
        )
        environment["GITHUB_OUTPUT"] = str(
            output_file(sandbox.outputs_directory, step_id)
        )
        result = run_shell(workspace, script, environment)
        completed.append(result)
        if result.returncode != 0:
            break
    return completed


def execute(
    tmp_path: Path, workspace: Path, facts: Facts, *, release: Release = NO_RELEASE
) -> ActionRun:
    """Run the action's script steps in order, stopping where a job would stop.

    Parameters
    ----------
    release
        The release the run observes and the mode it runs in. Its
        ``before_step`` hook, if any, acts between two of the steps.

    Returns
    -------
    ActionRun
        The completed steps, the outputs each step wrote, and the recorded
        calls.
    """
    sandbox = _sandbox(tmp_path, list(release.attached))
    completed = run_steps(
        workspace,
        facts,
        RunSetup(
            sandbox=sandbox,
            dry_run=release.dry_run,
            before_step=release.before_step,
        ),
    )
    return ActionRun(
        completed=tuple(completed),
        step_outputs=read_step_outputs(sandbox.outputs_directory),
        declared_root=sandbox.declared_root,
        gh_log=sandbox.gh_log.read_text(encoding="utf-8"),
        baselines=tuple(sandbox.baseline_log.read_text(encoding="utf-8").split()),
    )
