"""Exercise timing-mark commands and fixed failure reporting."""

import os
import subprocess  # ruff: ignore[suspicious-subprocess-import] - the CLI is exercised as a child.
import sys
import typing as typ

import pytest
from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

sys.path.insert(0, str(REPO_ROOT / "scripts"))

import kani_smoke_metrics as metrics

SCRIPT = REPO_ROOT / "scripts" / "kani_smoke_metrics.py"


def test_mark_commands_write_monotonic_values_in_the_state_directory(
    tmp_path: Path,
) -> None:
    """Expose separate start and end commands for workflow step placement."""
    state_directory = tmp_path / "state"
    environment = {
        **os.environ,
        "INPUT_STATE_DIRECTORY": str(state_directory),
    }
    for command, mark_name in (
        ("mark-start", metrics.MARK_START_FILE),
        ("mark-end", metrics.MARK_END_FILE),
    ):
        completed = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
            [sys.executable, str(SCRIPT), command],
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )
        assert completed.returncode == 0, completed.stderr
        value = int((state_directory / mark_name).read_text(encoding="ascii"))
        assert value >= 0, f"{command} must write a monotonic nanosecond value"


@pytest.mark.parametrize("mark", [metrics.mark_start, metrics.mark_end])
def test_mark_write_failure_warns_without_raising(
    tmp_path: Path, capsys: pytest.CaptureFixture[str], mark: cabc.Callable[..., bool]
) -> None:
    """Keep timing-file failures non-fatal to proof and mutation commands."""
    blocked_directory = tmp_path / "not-a-directory"
    blocked_directory.write_text("file", encoding="utf-8")
    assert not mark(blocked_directory, clock=lambda: 123), (
        "the marker must report that its file was not retained"
    )
    assert capsys.readouterr().err == metrics.WARNING_MARK_WRITE + "\n", (
        "a mark write failure must produce one fixed warning"
    )


@pytest.mark.parametrize("command", ["mark-start", "mark-end"])
def test_mark_cli_write_failure_exits_nonzero_with_fixed_warning(
    tmp_path: Path, command: str
) -> None:
    """Let the workflow capture failed mark commands without raw errors."""
    blocked_directory = tmp_path / "not-a-directory"
    blocked_directory.write_text("file", encoding="utf-8")
    environment = {
        **os.environ,
        "INPUT_STATE_DIRECTORY": str(blocked_directory / "state"),
    }
    completed = subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - fixed argv, no shell.
        [sys.executable, str(SCRIPT), command],
        env=environment,
        capture_output=True,
        text=True,
        check=False,
    )
    assert completed.returncode != 0, "a failed mark must reach the workflow shell"
    assert completed.stderr.endswith(metrics.WARNING_MARK_WRITE + "\n"), (
        "mark failures must use a fixed warning without paths or exceptions"
    )
