"""Exercise the command and path helpers shared by the CI helper scripts."""

import os
import typing as typ

import pytest
from ci_script_support import load_ci_script, write_executable

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox

support = load_ci_script("ci_support")


def test_run_returns_captured_output_for_an_allowlisted_program(
    cmd_mox: CmdMox,
) -> None:
    """A mocked program's stdout and exit code come back on the result."""
    cmd_mox.mock("frob").with_args("--flag", "value").returns(stdout="done\n")

    result = support.run(
        "frob",
        "--flag",
        "value",
        spec=support.RunSpec(support.catalogue("frob"), echo=False),
    )

    assert result.ok, result
    assert result.stdout == "done\n", "expected: result.stdout == 'done\n'"


def test_run_refuses_a_program_outside_the_catalogue() -> None:
    """The catalogue is the allowlist: an unlisted program never spawns."""
    from cuprum import UnknownProgramError

    with pytest.raises(UnknownProgramError):
        support.run(
            "frob", spec=support.RunSpec(support.catalogue("other"), echo=False)
        )


def test_run_overlays_env_on_the_child(cmd_mox: CmdMox) -> None:
    """A per-run env overlay reaches the child without touching os.environ."""
    seen: list[str | None] = []
    cmd_mox.mock("frob").runs(
        lambda inv: (seen.append(inv.env.get("CI_PROBE")) or "", "", 0)
    )

    support.run(
        "frob",
        spec=support.RunSpec(
            support.catalogue("frob"), env={"CI_PROBE": "yes"}, echo=False
        ),
    )

    assert seen == ["yes"], "expected: seen == ['yes']"
    assert "CI_PROBE" not in os.environ, "expected: 'CI_PROBE' not in os.environ"


def test_describe_failure_names_the_command_and_last_stderr_line(
    cmd_mox: CmdMox,
) -> None:
    """Failure text carries the argv, exit code, and the final stderr line."""
    cmd_mox.mock("frob").with_args("x").returns(
        stderr="first\nlast line\n", exit_code=3
    )

    result = support.run(
        "frob", "x", spec=support.RunSpec(support.catalogue("frob"), echo=False)
    )

    assert support.describe_failure(result) == "`frob x` exited 3: last line", (
        "expected: support.describe_failure(result) == '`frob x` exited 3: l..."
    )


def test_append_github_path_appends_one_line(tmp_path: pathlib.Path) -> None:
    """Each published directory is one line, appended after existing content."""
    github_path = tmp_path / "path"
    github_path.write_text("/existing\n", encoding="utf-8")

    support.append_github_path(tmp_path / "bin", github_path)

    assert (
        github_path.read_text(encoding="utf-8") == f"/existing\n{tmp_path / 'bin'}\n"
    ), "expected: github_path.read_text(encoding='utf-8') == f'/existing\n{..."


def test_prepend_path_puts_the_directory_first(
    monkeypatch: pytest.MonkeyPatch, tmp_path: pathlib.Path
) -> None:
    """The process PATH gains the directory ahead of everything else."""
    monkeypatch.setenv("PATH", "/usr/bin")

    support.prepend_path(tmp_path)

    assert os.environ["PATH"] == f"{tmp_path}{os.pathsep}/usr/bin", (
        "the directory must be first on PATH, ahead of the existing entries"
    )


def test_is_executable_file_rejects_directories_and_plain_files(
    tmp_path: pathlib.Path,
) -> None:
    """Only a regular file with the execute bit counts."""
    plain = tmp_path / "plain"
    plain.write_text("x", encoding="utf-8")
    executable = write_executable(tmp_path / "exe", "true")

    assert not support.is_executable_file(tmp_path), (
        "expected: not support.is_executable_file(tmp_path)"
    )
    assert not support.is_executable_file(plain), (
        "expected: not support.is_executable_file(plain)"
    )
    assert not support.is_executable_file(tmp_path / "missing"), (
        "expected: not support.is_executable_file(tmp_path / 'missing')"
    )
    assert support.is_executable_file(executable), (
        "expected: support.is_executable_file(executable)"
    )


@pytest.mark.skipif(os.geteuid() == 0, reason="root bypasses permission bits")
def test_is_executable_file_raises_for_a_path_it_cannot_inspect(
    tmp_path: pathlib.Path,
) -> None:
    """A path that exists but cannot be stat'd is an error, not a ``False``.

    Absence means "install the payload"; an unreadable path means the probe
    itself failed. Folding the second into the first would send the caller to
    a download that fails the same way, with the real cause -- the permissions
    on the cache volume -- never named.
    """
    blocked = tmp_path / "blocked"
    blocked.mkdir()
    write_executable(blocked / "tool", "true")
    original = blocked.stat().st_mode
    try:
        blocked.chmod(0o000)
        with pytest.raises(support.CiScriptError, match="cannot inspect"):
            support.is_executable_file(blocked / "tool")
    finally:
        # Restore before the temporary directory is torn down, so cleanup can
        # descend into it.
        blocked.chmod(original)
