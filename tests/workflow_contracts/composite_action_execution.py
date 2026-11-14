"""Hold what every harness that executes a composite action needs.

Two harnesses in this directory run a composite action's real shell bodies as
child processes: ``upload_declared_assets_harness`` for the publisher and
``verify_release_inventory_harness`` for the inventory verifier. What they share
is not the action's behaviour but the mechanics of pretending to be a runner:
reading ``GITHUB_OUTPUT``, checking the interpreter pin a step names, installing
an executable stand-in, and starting one step under Bash.

Keeping those here is what stops the two copies drifting. The output decoder is
the one that matters: a heredoc value that one harness decoded and the other
truncated would make a multi-line diagnostic pass in one suite and fail in the
other while both read the same action output.

Run via ``make test-workflow-contracts``.
"""

import re
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the actions' own steps are under test.
import typing as typ

from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    from pathlib import Path

#: The Makefile declaration every step must agree with when it names an
#: interpreter, read rather than restated so a pin bump changes one file.
BASELINE_PATTERN = r"^PYTHON_BASELINE \?= (\S+)$"


def write_stub(path: Path, body: str) -> None:
    """Install one executable stand-in for an external command."""
    path.write_text(body, encoding="utf-8")
    path.chmod(0o755)


def run_shell(
    workspace: Path, script: Path, environment: dict[str, str]
) -> subprocess.CompletedProcess[str]:
    """Run one step's shell body as a child process.

    Parameters
    ----------
    workspace
        The directory the runner would start the step in.
    script
        The file holding the step's ``run`` body.
    environment
        The environment the step inherits.

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
    entries these actions write never carry ``<<`` in a plain value, which is
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


def output_file(outputs_directory: Path, step_id: str) -> Path:
    """Return the ``GITHUB_OUTPUT`` file one manifest step writes.

    The file is created empty when first asked for, which is how a runner
    behaves: a step that writes nothing leaves an empty file rather than an
    absent one.

    Returns
    -------
    Path
        The file the step appends to.
    """
    path = outputs_directory / f"{step_id}.output"
    path.touch(exist_ok=True)
    return path


def read_step_outputs(
    outputs_directory: Path,
) -> dict[str, dict[str, str]]:
    """Read every step's ``GITHUB_OUTPUT`` entries, keyed by step id.

    Returns
    -------
    dict[str, dict[str, str]]
        Step id mapped to the entries that step wrote. A step that wrote
        nothing maps to an empty mapping.
    """
    return {
        path.stem: read_outputs(path) for path in outputs_directory.glob("*.output")
    }


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


__all__ = [
    "BASELINE_PATTERN",
    "interpreter_baseline",
    "output_file",
    "read_outputs",
    "read_step_outputs",
    "run_shell",
    "write_stub",
]
