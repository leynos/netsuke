"""Behavioural tests for ``scripts/ci/stage_test_shell.py``."""

import os
import pathlib
import sys
import typing as typ

import pytest
from ci_script_support import invoke, load_ci_script, write_executable

if typ.TYPE_CHECKING:
    from cmd_mox import CmdMox
    from cmd_mox.ipc import Invocation

script = load_ci_script("stage_test_shell")

GAWK_VERSION_LINE = "GNU Awk 5.3.1, API 4.0"


@pytest.fixture
def workflow_env(
    monkeypatch: pytest.MonkeyPatch, tmp_path: pathlib.Path
) -> pathlib.Path:
    """Provide the ambient GitHub Actions variables and an empty GITHUB_PATH."""
    runner_temp = tmp_path / "runner-temp"
    runner_temp.mkdir()
    github_path = tmp_path / "github-path"
    github_path.touch()
    monkeypatch.setenv("RUNNER_TEMP", str(runner_temp))
    monkeypatch.setenv("GITHUB_PATH", str(github_path))
    return tmp_path


@pytest.fixture
def system_gawk(
    monkeypatch: pytest.MonkeyPatch, tmp_path: pathlib.Path
) -> pathlib.Path:
    """Put a fake ``gawk`` on PATH behind an ``awk`` symlink, as Ubuntu does."""
    system_bin = tmp_path / "usr-bin"
    gawk = write_executable(system_bin / "gawk", f"echo '{GAWK_VERSION_LINE}'")
    (system_bin / "awk").symlink_to(gawk)
    monkeypatch.setenv("PATH", f"{system_bin}{os.pathsep}{os.environ['PATH']}")
    return gawk


def _apt_get(
    cmd_mox: CmdMox,
    *,
    update_exit: int = 0,
    install_exit: int = 0,
    expected_calls: int = 2,
) -> list[list[str]]:
    """Mock ``sudo apt-get``, recording the argument lists it received.

    The two subcommands fail independently, so a test can prove that an
    ``update`` failure is not masked by the ``install`` that would follow it,
    and vice versa. ``expected_calls`` is what cmd-mox verifies at teardown, so
    a failure that stops the script early passes ``1``: the count is an
    assertion about how far the script got, not a fixture detail.

    Returns
    -------
    list[list[str]]
        The argument lists recorded so far, which the live mock appends to as
        the script runs.
    """
    calls: list[list[str]] = []

    def handle(invocation: Invocation) -> tuple[str, str, int]:
        """Record the call and fail the subcommand the test selected."""
        calls.append(list(invocation.args))
        subcommand = "install" if "install" in invocation.args else "update"
        exit_code = install_exit if subcommand == "install" else update_exit
        return ("", f"E: {subcommand} failed\n" if exit_code else "", exit_code)

    cmd_mox.mock("sudo").runs(handle).times(expected_calls)
    return calls


def test_stages_gawk_as_a_regular_awk_and_publishes_the_directory(
    cmd_mox: CmdMox,
    workflow_env: pathlib.Path,
    system_gawk: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Cold run: apt installs gawk, a regular copy is staged, verified, published."""
    calls = _apt_get(cmd_mox)

    exit_code = invoke(script.app)

    assert exit_code == 0, "expected: exit_code == 0"
    assert calls == [
        ["apt-get", "update"],
        ["apt-get", "install", "--yes", "--no-install-recommends", "gawk"],
    ], "expected: calls == [ ['apt-get', 'update'], ['apt-get', 'install', ..."
    staged = workflow_env / "runner-temp" / "netsuke-test-bin" / "awk"
    assert staged.is_file(), "expected: staged.is_file()"
    assert not staged.is_symlink(), "the sandbox probe cannot follow a symlink"
    assert os.access(staged, os.X_OK), "expected: os.access(staged, os.X_OK)"
    assert staged.read_bytes() == system_gawk.read_bytes(), (
        "expected: staged.read_bytes() == system_gawk.read_bytes()"
    )
    github_path = (workflow_env / "github-path").read_text(encoding="utf-8")
    assert github_path == f"{staged.parent}\n", (
        "expected: github_path == f'{staged.parent}\n'"
    )
    assert GAWK_VERSION_LINE in capsys.readouterr().out, (
        "expected: GAWK_VERSION_LINE in capsys.readouterr().out"
    )


def test_reports_a_failed_package_install_and_stages_nothing(
    cmd_mox: CmdMox,
    workflow_env: pathlib.Path,
    system_gawk: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """An apt failure stops the step with its reason; nothing is published."""
    _apt_get(cmd_mox, install_exit=100)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    captured = capsys.readouterr()
    assert "installing gawk failed" in captured.err, (
        "expected: 'installing gawk failed' in captured.err"
    )
    assert "exited 100" in captured.err, "expected: 'exited 100' in captured.err"
    assert not (workflow_env / "runner-temp" / "netsuke-test-bin").exists(), (
        "expected: not (workflow_env / 'runner-temp' / 'netsuke-test-bin').e..."
    )
    assert not (workflow_env / "github-path").read_text(encoding="utf-8"), (
        "expected: not (workflow_env / 'github-path').read_text(encoding='ut..."
    )


def test_reports_a_failed_package_index_update_and_stages_nothing(
    cmd_mox: CmdMox,
    workflow_env: pathlib.Path,
    system_gawk: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """An ``apt-get update`` failure stops the step before ``install`` runs.

    The update is the first call, so this is the failure that reaches the
    least code. Folding it into the ``install`` case would let an update that
    is ignored entirely -- and an install attempted against a stale index --
    pass unnoticed, which is why the two are separate cases.
    """
    calls = _apt_get(cmd_mox, update_exit=100, expected_calls=1)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    assert calls == [["apt-get", "update"]], (
        "a failed update must stop the step before install runs"
    )
    captured = capsys.readouterr()
    assert "installing gawk failed" in captured.err, (
        "expected: 'installing gawk failed' in captured.err"
    )
    assert "update failed" in captured.err, (
        "expected: the diagnostic to name the failing subcommand"
    )
    assert not (workflow_env / "runner-temp" / "netsuke-test-bin").exists(), (
        "expected: a failed update stages nothing"
    )
    assert not (workflow_env / "github-path").read_text(encoding="utf-8"), (
        "expected: a failed update publishes nothing"
    )


def test_reports_a_staged_copy_that_does_not_run(
    cmd_mox: CmdMox,
    workflow_env: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A staged copy that fails ``awk --version`` fails the step and is named.

    Staging is a byte copy of a binary the script did not build, so the
    verification run is the only evidence the sandbox will be able to execute
    it. A copy that lands but does not run must fail here, where the diagnostic
    names the file, rather than later inside the test sandbox.

    ``workflow_env`` is the temporary directory itself, so the fake system
    ``gawk`` is staged inside it alongside the workflow files rather than in a
    second directory that would need its own cleanup.
    """
    system_bin = workflow_env / "usr-bin"
    # `shutil.which` finds it and the copy succeeds; running the copy does not.
    write_executable(system_bin / "gawk", "exit 3")
    monkeypatch.setenv("PATH", f"{system_bin}{os.pathsep}{os.environ['PATH']}")
    _apt_get(cmd_mox)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    captured = capsys.readouterr()
    assert "the staged awk does not run" in captured.err, (
        "expected: 'the staged awk does not run' in captured.err"
    )
    assert "exited 3" in captured.err, "expected: the diagnostic to carry the exit code"
    staged = workflow_env / "runner-temp" / "netsuke-test-bin" / "awk"
    assert staged.is_file(), "the staged copy itself is left in place"
    assert (workflow_env / "github-path").read_text(encoding="utf-8") == (
        f"{staged.parent}\n"
    ), "the directory is published before verification, so it stays published"


def test_reports_a_package_that_leaves_no_binary_on_path(
    cmd_mox: CmdMox,
    workflow_env: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A successful install that provides no ``gawk`` is named, not staged."""
    shim_dir = cmd_mox.environment.shim_dir
    assert shim_dir is not None, "cmd-mox must provide its shim directory"
    # Keep the shims (and the interpreter they run under) reachable while
    # hiding every real binary from the search.
    interpreter_dir = pathlib.Path(sys.executable).parent
    monkeypatch.setenv(
        "PATH",
        os.pathsep.join([
            str(workflow_env / "empty-bin"),
            str(shim_dir),
            str(interpreter_dir),
        ]),
    )
    _apt_get(cmd_mox)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    assert "gawk is not on PATH after installation" in capsys.readouterr().err, (
        "expected: 'gawk is not on PATH after installation' in capsys.readou..."
    )


def test_a_staged_copy_printing_only_whitespace_still_succeeds(
    cmd_mox: CmdMox,
    workflow_env: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A staged copy that runs but says nothing is staged, not an IndexError.

    ``verify_staged`` returns the first line as a courtesy report, so output
    that strips to nothing must yield the empty string. Indexing the stripped
    result before splitting it would raise ``IndexError``, which the entry
    point does not catch, and the step would fail with a traceback naming
    neither the staged file nor the reason.
    """
    system_bin = workflow_env / "usr-bin"
    # Runs successfully and exits 0, so the version probe passes.
    write_executable(system_bin / "gawk", "printf '  \\n'")
    monkeypatch.setenv("PATH", f"{system_bin}{os.pathsep}{os.environ['PATH']}")
    _apt_get(cmd_mox)

    exit_code = invoke(script.app)

    assert exit_code == 0, "expected: exit_code == 0"
    out = capsys.readouterr().out
    assert "Traceback" not in out, "the step must not fail with a traceback"
    staged = workflow_env / "runner-temp" / "netsuke-test-bin" / "awk"
    assert staged.is_file(), "expected: staged.is_file()"
    assert out.rstrip().endswith(f"staged {system_bin / 'gawk'} as {staged}:"), (
        "the report must end with the empty version line"
    )


def test_honours_package_and_staging_name_inputs(
    cmd_mox: CmdMox,
    workflow_env: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
) -> None:
    """INPUT_PACKAGE and INPUT_STAGING_NAME select the package and directory."""
    system_bin = tmp_path / "usr-bin"
    write_executable(system_bin / "mawk", "echo 'mawk 1.3'")
    monkeypatch.setenv("PATH", f"{system_bin}{os.pathsep}{os.environ['PATH']}")
    monkeypatch.setenv("INPUT_PACKAGE", "mawk")
    monkeypatch.setenv("INPUT_STAGING_NAME", "custom-bin")
    calls = _apt_get(cmd_mox)

    exit_code = invoke(script.app)

    assert exit_code == 0, "expected: exit_code == 0"
    assert calls[1][-1] == "mawk", "expected: calls[1][-1] == 'mawk'"
    assert (workflow_env / "runner-temp" / "custom-bin" / "awk").is_file(), (
        "expected: (workflow_env / 'runner-temp' / 'custom-bin' / 'awk').is_..."
    )
