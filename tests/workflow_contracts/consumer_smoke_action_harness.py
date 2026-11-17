"""Execute the consumer-smoke action's own shell steps outside GitHub.

The suites around ``.github/actions/consumer-smoke`` read the manifest as text:
they assert that the job carries no credential, that each step names the CLI,
and that the script around each invocation has the accepted shape. None of them
runs a step, so a flag spelled wrongly, an input read under a name the action
never exports, or a report step that reads a log it cannot parse can satisfy
every structural check and still fail on a runner.

This module closes that gap. Each ``run`` body is taken from the manifest rather
than restated here, so the command lines, the environment each step is handed,
and the phase-reporting loop are held to account as written. ``uv`` is stubbed
at the path boundary and hands the remainder to the real interpreter, so the
checked-in CLI runs for real; ``curl`` is stubbed so no request leaves the host,
and the bytes it serves are the fixtures a case published. The package record,
the trusted digest record, the downloaded bodies, the extracted binary, and the
two artefacts the verifier writes are all real files.

Three modules carry the pieces: the manifest readers and the expression
resolver in ``consumer_smoke_action_manifest.py``, the candidate and host
fixtures in ``consumer_smoke_action_fixtures.py``, and the cases in
``consumer_smoke_action_execution_test.py``.

Run via ``make test-workflow-contracts``.
"""

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
from composite_action_stubs import UV_STUB
from consumer_smoke_action_fixtures import (
    COMMIT,
    CURL_STUB,
    VERIFIED,
    Published,
    Smoke,
    publish,
)
from consumer_smoke_action_manifest import (
    REPORT_STEP,
    manifest_step_id,
    should_run,
    smoke_steps,
    step_environment,
)
from release_publish_path_consumer_shape import SMOKE_ACTION_TARGET
from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    import subprocess
    from pathlib import Path


@dataclasses.dataclass(frozen=True, slots=True)
class Sandbox:
    """Hold the paths one executed run writes and reads.

    Attributes
    ----------
    stubs
        The directory holding the ``uv`` and ``curl`` stand-ins.
    scratch
        A writable home for the child processes, also holding their scripts.
    outputs_directory
        One ``GITHUB_OUTPUT`` file per step, so a step's entries can be read
        back as that step's alone.
    curl_log
        Every URL the stand-in was asked for, one per line.
    baseline_log
        Every interpreter the ``uv`` stand-in was told to use.
    """

    stubs: Path
    scratch: Path
    outputs_directory: Path
    curl_log: Path
    baseline_log: Path


@dataclasses.dataclass(frozen=True, slots=True)
class ActionRun:
    """Report what executing the action's steps did.

    Attributes
    ----------
    completed
        Each step's completed process, in execution order. A step a runner
        would have skipped is absent, so the count names where the run stopped.
    step_outputs
        The ``GITHUB_OUTPUT`` entries each step wrote, keyed by the step's
        manifest id.
    work_dir
        The directory the action was told to keep its records in.
    curl_log
        Every URL the ``curl`` stand-in was asked for.
    baselines
        The interpreter each ``uv`` invocation was told to use.
    """

    completed: tuple[subprocess.CompletedProcess[str], ...]
    step_outputs: dict[str, dict[str, str]]
    work_dir: Path
    curl_log: tuple[str, ...]
    baselines: tuple[str, ...]

    @property
    def outputs(self) -> dict[str, str]:
        """The entries the report step wrote, which are the action's outputs."""
        return self.step_outputs[manifest_step_id(REPORT_STEP)]

    def verdict(self) -> dict[str, object] | None:
        """Return the verdict record, or ``None`` when the run wrote none."""
        return _read_json(self.work_dir / "verdict.json")


def _read_json(path: Path) -> dict[str, object] | None:
    """Return a JSON object a run wrote, or ``None`` when it is absent."""
    if not path.is_file():
        return None
    return json.loads(path.read_text(encoding="utf-8"))


def _sandbox(root: Path) -> Sandbox:
    """Create the stubs and logs one run needs.

    Returns
    -------
    Sandbox
        The paths the run writes.
    """
    stubs = root / "stubs"
    stubs.mkdir()
    write_stub(stubs / "uv", UV_STUB)
    write_stub(stubs / "curl", CURL_STUB)
    scratch = root / "scratch"
    scratch.mkdir()
    baseline_log = root / "uv-baselines.txt"
    baseline_log.touch()
    curl_log = root / "curl.log"
    curl_log.touch()
    outputs_directory = root / "github-output"
    outputs_directory.mkdir()
    return Sandbox(
        stubs=stubs,
        scratch=scratch,
        outputs_directory=outputs_directory,
        curl_log=curl_log,
        baseline_log=baseline_log,
    )


def _environment(sandbox: Sandbox, published: Published) -> dict[str, str]:
    """Return the environment every step inherits.

    ``GITHUB_OUTPUT`` is deliberately absent: a step's output file is that
    step's own, and :func:`execute` supplies it per step.

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
        "UV_BASELINE_LOG": str(sandbox.baseline_log),
        "UV_PYTHON": sys.executable,
        "CURL_LOG": str(sandbox.curl_log),
        "CURL_FIXTURES": str(published.fixtures),
    }


def _default_inputs(published: Published) -> dict[str, str]:
    """Return the inputs a case that overrides none supplies.

    Returns
    -------
    dict[str, str]
        The six declared inputs, naming this run's evidence and workspace.
    """
    return {
        "identity-path": str(published.identity),
        "asset-digest-record": str(published.trusted),
        "repository": "leynos/netsuke",
        "commit": COMMIT,
        "target": SMOKE_ACTION_TARGET,
        "work-dir": str(published.work_dir),
    }


@dataclasses.dataclass(slots=True)
class _Attempt:
    """Hold what one executed run carries from one step to the next."""

    sandbox: Sandbox
    published: Published
    inputs: dict[str, str]
    base_environment: dict[str, str]
    steps_dir: Path
    outputs: dict[str, dict[str, str]] = dataclasses.field(default_factory=dict)
    outcomes: dict[str, str] = dataclasses.field(default_factory=dict)
    completed: list[subprocess.CompletedProcess[str]] = dataclasses.field(
        default_factory=list
    )


def _start_step(step: dict[str, object], number: int, attempt: _Attempt) -> None:
    """Run one step, or record that a runner would skip it."""
    step_id = manifest_step_id(str(step.get("name", "")))
    outputs_path = output_file(attempt.sandbox.outputs_directory, step_id)
    if not should_run(
        step, outcomes=attempt.outcomes, previous=list(attempt.outcomes.values())
    ):
        attempt.outcomes[step_id] = "skipped"
        return
    script = attempt.steps_dir / f"{number}.sh"
    script.write_text(str(step.get("run", "")), encoding="utf-8")
    environment = attempt.base_environment | step_environment(
        step,
        inputs=attempt.inputs,
        outputs=attempt.outputs,
        outcomes=attempt.outcomes,
    )
    environment["GITHUB_OUTPUT"] = str(outputs_path)
    result = run_shell(REPO_ROOT, script, environment)
    attempt.outcomes[step_id] = "success" if result.returncode == 0 else "failure"
    attempt.completed.append(result)
    attempt.outputs = read_step_outputs(attempt.sandbox.outputs_directory)


def execute(tmp_path: Path, *, smoke: Smoke = VERIFIED) -> ActionRun:
    """Run the action's steps in order, stopping where a runner would.

    Parameters
    ----------
    tmp_path
        The directory the evidence, the served bytes, and the run's records
        are written under.
    smoke
        The candidate the host publishes and the inputs a case overrides.

    Returns
    -------
    ActionRun
        The completed steps, the outputs each step wrote, and the calls the
        stand-ins recorded.

    Notes
    -----
    Each step runs with the repository root as its working directory, because
    that is the tree a runner's checkout puts the action in and the checked-in
    CLI resolves its default staging configuration and manifest from there.
    Nothing the steps write lands in that tree: the work directory, the
    downloads, and the extracted binary are all under ``tmp_path``.
    """
    published = publish(tmp_path, smoke)
    sandbox = _sandbox(tmp_path)
    steps_dir = sandbox.scratch / "steps"
    steps_dir.mkdir()
    attempt = _Attempt(
        sandbox=sandbox,
        published=published,
        inputs=_default_inputs(published) | smoke.inputs,
        base_environment=_environment(sandbox, published),
        steps_dir=steps_dir,
    )
    for number, step in enumerate(smoke_steps()):
        if smoke.before_step is not None:
            smoke.before_step(number, published.work_dir)
        _start_step(step, number, attempt)
    return ActionRun(
        completed=tuple(attempt.completed),
        step_outputs=read_step_outputs(sandbox.outputs_directory),
        work_dir=published.work_dir,
        curl_log=tuple(sandbox.curl_log.read_text(encoding="utf-8").splitlines()),
        baselines=tuple(sandbox.baseline_log.read_text(encoding="utf-8").split()),
    )


__all__ = [
    "ActionRun",
    "Sandbox",
    "execute",
]
