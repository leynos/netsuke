"""Execute the inventory verifier's own shell steps outside GitHub.

The other suites around ``.github/actions/verify-release-inventory`` read its
manifest as text and the workflow as data: they assert which script each step
names, that no ``run`` block carries a GitHub expression, that the API calls are
GETs, and that the caller wires the trusted artefact correctly. None of them
runs the steps, so a step that spells a flag wrongly, passes a path its CLI
cannot open, or writes its output under a name its caller does not read can
satisfy every structural check and still fail on a runner.

This module closes that gap. Each ``run`` body is taken from the manifest (read
by ``verify_release_inventory_manifest.py``) rather than restated here, so the
flag spelling, the argument order, and the ``verified-record-path`` output are
held to account as written. ``uv`` and ``gh`` are stubbed at the path boundary,
so the real CLIs execute without downloading an interpreter or contacting
GitHub; everything else -- the publisher's build tree, its declaration and
trusted digest record, the attachments served for download, and the verified
record -- is written and read for real.

The workspace the steps run in is assembled per run: the repository's own
``scripts`` are copied in, so a step that resolved a script through a path the
checkout does not provide fails here rather than on a runner. What a run is
handed is described by ``verify_release_inventory_scenario.py``; the cases that
use this harness are the two ``verify_release_inventory_*_test`` modules.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import json
import shutil
import sys
import typing as typ

import release_digest_files
import release_digest_record_files
from composite_action_execution import read_outputs, run_shell, write_stub
from composite_action_stubs import UV_STUB
from verify_release_inventory_manifest import run_bodies, step_environment
from verify_release_inventory_scenario import (
    INVENTORY_DIRECTORY,
    Publisher,
    Remote,
    Scenario,
)
from verify_release_inventory_stubs import GH_API_HELPER, GH_API_STUB
from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    import subprocess
    from pathlib import Path


@dataclasses.dataclass(frozen=True, slots=True)
class ActionRun:
    """Report what executing the action's steps did.

    Attributes
    ----------
    completed
        Each step's completed process, in execution order. Fewer than
        :data:`verify_release_inventory_manifest.RUN_STEPS` when a step failed
        and the job stopped there.
    outputs
        The ``GITHUB_OUTPUT`` entries the verification step wrote.
    workspace
        The directory the steps ran in.
    download_root
        The directory the attachments were downloaded into.
    output_path
        The verified record the verification step writes.
    gh_log
        Every recorded ``gh`` invocation, one per line.
    baselines
        The interpreter each ``uv`` invocation was told to use.
    trusted_inputs
        The declaration and digest record as the harness staged them, keyed by
        file name. A case compares them with the run's copies to show a
        refusal left the publisher's evidence untouched.
    """

    completed: tuple[subprocess.CompletedProcess[str], ...]
    outputs: dict[str, str]
    workspace: Path
    download_root: Path
    output_path: Path
    gh_log: str
    baselines: tuple[str, ...]
    trusted_inputs: dict[str, bytes]

    @property
    def downloaded(self) -> tuple[str, ...]:
        """The names present in the download directory, sorted."""
        if not self.download_root.is_dir():
            return ()
        return tuple(sorted(path.name for path in self.download_root.iterdir()))

    @property
    def plan(self) -> tuple[tuple[str, str], ...]:
        """The download plan the listing step wrote, as identifier and name.

        Returns
        -------
        tuple[tuple[str, str], ...]
            One pair per planned download, in the order the plan lists them.
        """
        path = self.workspace / INVENTORY_DIRECTORY / "download-plan.txt"
        pairs: list[tuple[str, str]] = []
        for line in path.read_text(encoding="utf-8").splitlines():
            if line:
                identifier, name = line.split("\t", 1)
                pairs.append((identifier, name))
        return tuple(pairs)

    @property
    def verified_record(self) -> dict[str, str]:
        """The verified digest record the verification step wrote.

        Returns
        -------
        dict[str, str]
            Asset name mapped to the digest that was confirmed for it.
        """
        decoded: dict[str, str] = json.loads(
            self.output_path.read_text(encoding="utf-8")
        )["assets"]
        return decoded


def _stage_scripts(workspace: Path) -> None:
    """Copy the repository's release scripts into the run's workspace.

    The steps invoke ``scripts/...`` relatively, so the workspace has to carry
    them. Copying rather than pointing at the checkout keeps a step that wrote a
    file beside its script from writing into the repository.
    """
    (workspace / "scripts").mkdir(parents=True)
    for module in sorted((REPO_ROOT / "scripts").glob("*.py")):
        shutil.copy2(module, workspace / "scripts" / module.name)


def _stage_trusted(
    workspace: Path, built: Path, publisher: Publisher
) -> dict[str, bytes]:
    """Write the publisher's declaration and trusted digest record.

    Returns
    -------
    dict[str, bytes]
        The staged files keyed by name, read back after writing so a case can
        compare a run's copies with exactly what the harness put there.
    """
    built.mkdir(parents=True)
    for name, body in publisher.files.items():
        _ = (built / name).write_bytes(body)
    trusted = workspace / "trusted"
    trusted.mkdir()
    _ = (trusted / "declaration.txt").write_text(
        "\n".join(publisher.declared_names) + "\n", encoding="utf-8"
    )
    digests = {
        name: release_digest_files.digest_of_file(built / name)
        for name in publisher.recorded_names
    }
    release_digest_record_files.write_digest_record(digests, trusted / "trusted.json")
    return {
        name: (trusted / name).read_bytes()
        for name in ("declaration.txt", "trusted.json")
    }


def _stage_sources(sources: Path, remote: Remote) -> Path:
    """Write each attachment's bytes and return the state the stub serves.

    Returns
    -------
    Path
        The state file the ``gh`` stand-in answers from.
    """
    sources.mkdir()
    for asset in remote.assets:
        _ = (sources / str(asset.identifier)).write_bytes(asset.body)
    first = remote.releases[0].identifier if remote.releases else 0
    state = {
        "release_id": first,
        "releases": [
            {"id": entry.identifier, "tag_name": entry.tag, "draft": entry.draft}
            for entry in remote.releases
        ],
        "assets": [
            {
                "id": asset.identifier,
                "name": asset.name,
                "state": asset.state,
                "size": len(asset.body) if asset.size is None else asset.size,
                "source": str(sources / str(asset.identifier)),
                "download_fails": asset.download_fails,
            }
            for asset in remote.assets
        ],
    }
    destination = sources.parent / "gh-state.json"
    _ = destination.write_text(json.dumps(state), encoding="utf-8")
    return destination


@dataclasses.dataclass(frozen=True, slots=True)
class Sandbox:
    """Hold the paths one executed run writes and reads.

    Attributes
    ----------
    stubs
        The directory holding the ``uv`` and ``gh`` stand-ins.
    scratch
        A writable home for the child processes, also holding their scripts.
    output_file
        The ``GITHUB_OUTPUT`` file the steps append to.
    gh_log
        Every ``gh`` invocation the stub recorded.
    state
        The release state the ``gh`` stub answers from.
    helper
        The Python program the ``gh`` stub runs to answer a call.
    baseline_log
        Every interpreter the ``uv`` stub was told to use.
    workspace
        The directory the steps run in.
    trusted_inputs
        The declaration and digest record as the harness staged them, keyed by
        file name, so a case can detect any change a run made to them.
    """

    stubs: Path
    scratch: Path
    output_file: Path
    gh_log: Path
    state: Path
    helper: Path
    baseline_log: Path
    workspace: Path
    trusted_inputs: dict[str, bytes]


def _run_root(tmp_path: Path) -> Path:
    """Create the directory one run owns, apart from every earlier run.

    A case that proves a refusal after a good run would otherwise inherit the
    first run's log, listings, and downloaded bytes.

    Returns
    -------
    Path
        The directory this run keeps its stubs, state, and workspace in.
    """
    runs = tmp_path / "runs"
    runs.mkdir(exist_ok=True)
    root = runs / f"run-{len(tuple(runs.iterdir()))}"
    root.mkdir()
    return root


def _install_stubs(root: Path) -> Path:
    """Install the ``uv`` and ``gh`` stand-ins and the helper they run.

    Returns
    -------
    Path
        The directory holding the stand-ins.
    """
    stubs = root / "stubs"
    stubs.mkdir()
    write_stub(stubs / "uv", UV_STUB)
    write_stub(stubs / "gh", GH_API_STUB)
    # The helper sits beside the stubs but is run, not exec'd from PATH, so it
    # needs no execute bit and cannot be picked up as a command by accident.
    _ = (stubs / "gh_api_state.py").write_text(GH_API_HELPER, encoding="utf-8")
    return stubs


def _sandbox(tmp_path: Path, scenario: Scenario) -> Sandbox:
    """Create the stubs, the state, and the workspace one run needs.

    Returns
    -------
    Sandbox
        The paths the run writes and reads.
    """
    root = _run_root(tmp_path)
    stubs = _install_stubs(root)
    helper = stubs / "gh_api_state.py"
    scratch = root / "scratch"
    scratch.mkdir()
    baseline_log = root / "uv-baselines.txt"
    baseline_log.touch()
    gh_log = root / "gh.log"
    gh_log.touch()
    state = _stage_sources(root / "sources", scenario.remote)
    output_file = root / "github-output"
    output_file.touch()
    workspace = root / "workspace"
    _stage_scripts(workspace)
    trusted_inputs = _stage_trusted(workspace, root / "build", scenario.publisher)
    return Sandbox(
        stubs=stubs,
        scratch=scratch,
        output_file=output_file,
        gh_log=gh_log,
        state=state,
        helper=helper,
        baseline_log=baseline_log,
        workspace=workspace,
        trusted_inputs=trusted_inputs,
    )


def _environment(sandbox: Sandbox) -> dict[str, str]:
    """Return the environment every step inherits.

    The stubs come first on ``PATH`` so the steps reach them rather than any
    real install, and ``HOME`` and ``TMPDIR`` point at scratch so nothing writes
    into the reader's own caches.

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
        "GITHUB_OUTPUT": str(sandbox.output_file),
        "GH_LOG": str(sandbox.gh_log),
        "GH_STATE": str(sandbox.state),
        "GH_HELPER": str(sandbox.helper),
        "GH_PYTHON": sys.executable,
        "UV_BASELINE_LOG": str(sandbox.baseline_log),
        "UV_PYTHON": sys.executable,
    }


def _run_steps(
    sandbox: Sandbox, inventory_root: Path, values: dict[str, str]
) -> list[subprocess.CompletedProcess[str]]:
    """Run each script step in order, stopping where a job would stop.

    A failed step stops the loop because GitHub would not start the next one;
    the caller can then tell a refusal from a later fault by how many steps
    completed.

    Returns
    -------
    list[subprocess.CompletedProcess[str]]
        Each completed step, in execution order.
    """
    steps = sandbox.scratch / "steps"
    steps.mkdir()
    base_env = _environment(sandbox)
    completed: list[subprocess.CompletedProcess[str]] = []
    for number, step in enumerate(run_bodies()):
        script = steps / f"{number}.sh"
        _ = script.write_text(str(step.get("run", "")), encoding="utf-8")
        environment = base_env | step_environment(
            step, values=values, inventory_root=inventory_root
        )
        result = run_shell(sandbox.workspace, script, environment)
        completed.append(result)
        if result.returncode != 0:
            break
    return completed


def execute(tmp_path: Path, scenario: Scenario) -> ActionRun:
    """Run the action's script steps in order, stopping where a job would stop.

    Returns
    -------
    ActionRun
        The completed steps, the outputs they wrote, and the recorded calls.
    """
    sandbox = _sandbox(tmp_path, scenario)
    inventory_root = sandbox.workspace / INVENTORY_DIRECTORY
    scenario.prepare(sandbox.workspace)
    completed = _run_steps(sandbox, inventory_root, scenario.inputs.values())
    return ActionRun(
        completed=tuple(completed),
        outputs=read_outputs(sandbox.output_file),
        workspace=sandbox.workspace,
        download_root=sandbox.workspace / scenario.inputs.download_directory,
        output_path=sandbox.workspace / scenario.inputs.output_path,
        gh_log=sandbox.gh_log.read_text(encoding="utf-8"),
        baselines=tuple(sandbox.baseline_log.read_text(encoding="utf-8").split()),
        trusted_inputs=sandbox.trusted_inputs,
    )
