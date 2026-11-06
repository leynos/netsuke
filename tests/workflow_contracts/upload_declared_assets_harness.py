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

import dataclasses
import json
import re
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the action's own steps are under test.
import sys
import typing as typ

from upload_declared_assets_manifest import run_bodies, step_environment
from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    from pathlib import Path

    from upload_declared_assets_staging import Facts

#: The Makefile declaration every step must agree with when it names an
#: interpreter, read rather than restated so a pin bump changes one file.
BASELINE_PATTERN = r"^PYTHON_BASELINE \?= (\S+)$"

UV_STUB = """\
#!/bin/sh
# Stands in for `uv run --no-project --python <baseline>`, which fetches the
# baseline interpreter on demand. The subject here is the action's own script,
# so the prefix is checked as spelled and the remainder is handed to the test's
# interpreter. Nothing downloads, so nothing needs the network.
set -eu
test "$1" = run
test "$2" = --no-project
test "$3" = --python
printf '%s\\n' "$4" >> "$UV_BASELINE_LOG"
shift 4
exec "$UV_PYTHON" "$@"
"""

GH_STUB = """\
#!/bin/sh
# Stands in for the GitHub CLI. The listing is served from a file so a case can
# describe a release that already holds assets; every invocation is recorded so
# a case can assert which transfers were sent, and with which flags.
set -eu
printf '%s\\n' "$*" >> "$GH_LOG"
case "${1:-} ${2:-}" in
  "release view") cat "$GH_ATTACHED" ;;
  "release download") exit 1 ;;
  "release upload") exit 0 ;;
  *)
    printf 'unexpected gh invocation: %s\\n' "$*" >&2
    exit 1
    ;;
esac
"""


@dataclasses.dataclass(frozen=True, slots=True)
class ActionRun:
    """Report what executing the action's steps did.

    Attributes
    ----------
    completed
        Each step's completed process, in execution order. Fewer than
        :data:`RUN_STEPS` when a step failed and the job stopped there.
    outputs
        The ``GITHUB_OUTPUT`` entries the upload step wrote.
    declared_root
        The directory the action was told to keep its records in.
    gh_log
        Every recorded ``gh`` invocation, one per line.
    baselines
        The interpreter each ``uv`` invocation was told to use.
    """

    completed: tuple[subprocess.CompletedProcess[str], ...]
    outputs: dict[str, str]
    declared_root: Path
    gh_log: str
    baselines: tuple[str, ...]

    @property
    def payload(self) -> Path:
        """The directory the selection step installed the payload in."""
        return self.declared_root / "selected" / "payload"


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
    attached
        The release listing the ``gh`` stub serves.
    baseline_log
        Every interpreter the ``uv`` stub was told to use.
    declared_root
        The directory the action keeps its records in.
    """

    stubs: Path
    scratch: Path
    output_file: Path
    gh_log: Path
    attached: Path
    baseline_log: Path
    declared_root: Path


def _write_stub(path: Path, body: str) -> None:
    """Install one executable stand-in for an external command."""
    path.write_text(body, encoding="utf-8")
    path.chmod(0o755)


def _take_heredoc(
    lines: list[str], start: int, header: str, outputs: dict[str, str]
) -> int:
    """Decode the heredoc output whose value lines begin at ``start``.

    Returns
    -------
    int
        The index just past the closing delimiter.
    """
    name, _marker, delimiter = header.partition("<<")
    end = start
    while end < len(lines) and lines[end] != delimiter:
        end += 1
    outputs[name] = "\n".join(lines[start:end])
    return end + 1


def read_outputs(path: Path) -> dict[str, str]:
    """Decode a ``GITHUB_OUTPUT`` file, including its heredoc values.

    A heredoc value is written as ``name<<DELIMITER``, one or more lines, then
    the delimiter again, so a multi-line diagnostic survives the encoding. The
    entries this action writes never carry ``<<`` in a plain value, which is
    what lets the two forms be told apart by the marker alone.

    Returns
    -------
    dict[str, str]
        Output name mapped to its decoded value.
    """
    outputs: dict[str, str] = {}
    lines = path.read_text(encoding="utf-8").splitlines()
    index = 0
    while index < len(lines):
        line = lines[index]
        index += 1
        if "<<" in line:
            index = _take_heredoc(lines, index, line, outputs)
            continue
        name, _separator, plain = line.partition("=")
        outputs[name] = plain
    return outputs


def interpreter_baseline() -> str:
    """Return the interpreter the Makefile pins, which every step must name.

    Returns
    -------
    str
        The value of ``PYTHON_BASELINE`` in the Makefile.
    """
    text = (REPO_ROOT / "Makefile").read_text(encoding="utf-8")
    match = re.search(BASELINE_PATTERN, text, flags=re.MULTILINE)
    assert match is not None, "the Makefile must declare PYTHON_BASELINE ?="
    return match.group(1)


def _sandbox(tmp_path: Path, attached: list[dict[str, str]] | None) -> Sandbox:
    """Create the stubs and logs one run needs.

    Returns
    -------
    Sandbox
        The paths the run writes.
    """
    stubs = tmp_path / "stubs"
    stubs.mkdir()
    _write_stub(stubs / "uv", UV_STUB)
    _write_stub(stubs / "gh", GH_STUB)
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    baseline_log = tmp_path / "uv-baselines.txt"
    baseline_log.touch()
    gh_log = tmp_path / "gh.log"
    gh_log.touch()
    listing = tmp_path / "gh-attached.json"
    listing.write_text(json.dumps({"assets": attached or []}), encoding="utf-8")
    output_file = tmp_path / "github-output"
    output_file.touch()
    return Sandbox(
        stubs=stubs,
        scratch=scratch,
        output_file=output_file,
        gh_log=gh_log,
        attached=listing,
        baseline_log=baseline_log,
        declared_root=tmp_path / "runner" / "declared-assets",
    )


def _environment(sandbox: Sandbox) -> dict[str, str]:
    """Return the environment every step inherits.

    The stubs come first on ``PATH`` so the steps reach them rather than any
    real install, and ``HOME`` and ``TMPDIR`` point at scratch so nothing
    writes into the reader's own caches.

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
        "GITHUB_RUN_ID": "4242",
        "GH_LOG": str(sandbox.gh_log),
        "GH_ATTACHED": str(sandbox.attached),
        "UV_BASELINE_LOG": str(sandbox.baseline_log),
        "UV_PYTHON": sys.executable,
    }


def _run_one(
    workspace: Path, script: Path, environment: dict[str, str]
) -> subprocess.CompletedProcess[str]:
    """Run one step's shell body as a child process.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The completed step.
    """
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the step body is the shell script.
        ["bash", str(script)],  # ruff: ignore[start-process-with-partial-path] - resolved from the fixed PATH.
        cwd=workspace,
        check=False,
        env=environment,
        text=True,
        capture_output=True,
    )


def run_steps(
    workspace: Path, facts: Facts, sandbox: Sandbox
) -> list[subprocess.CompletedProcess[str]]:
    """Run each script step in order, stopping where a job would stop.

    A failed step stops the loop because GitHub would not start the next one;
    the caller can then tell a refusal from a later upload fault by how many
    steps completed.

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
        script.write_text(str(step.get("run", "")), encoding="utf-8")
        environment = base_env | step_environment(
            step, facts=facts, declared_root=sandbox.declared_root
        )
        result = _run_one(workspace, script, environment)
        completed.append(result)
        if result.returncode != 0:
            break
    return completed


def execute(
    tmp_path: Path,
    workspace: Path,
    facts: Facts,
    *,
    attached: list[dict[str, str]] | None = None,
) -> ActionRun:
    """Run the action's script steps in order, stopping where a job would stop.

    Returns
    -------
    ActionRun
        The completed steps, the outputs they wrote, and the recorded calls.
    """
    sandbox = _sandbox(tmp_path, attached)
    completed = run_steps(workspace, facts, sandbox)
    return ActionRun(
        completed=tuple(completed),
        outputs=read_outputs(sandbox.output_file),
        declared_root=sandbox.declared_root,
        gh_log=sandbox.gh_log.read_text(encoding="utf-8"),
        baselines=tuple(sandbox.baseline_log.read_text(encoding="utf-8").split()),
    )
