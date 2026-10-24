"""Behavioural tests for ``scripts/ci/discard_instrumented_tree.py``."""

import typing as typ

import pytest
from ci_script_support import invoke, load_ci_script

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox
    from cmd_mox.ipc import Invocation

script = load_ci_script("discard_instrumented_tree")


@pytest.fixture
def target(monkeypatch: pytest.MonkeyPatch, tmp_path: pathlib.Path) -> pathlib.Path:
    """Provide a Cargo target directory with both instrumented trees."""
    target = tmp_path / "target"
    (target / "llvm-cov-target" / "debug").mkdir(parents=True)
    (target / "llvm-cov-target" / "debug" / "obj").write_bytes(b"x")
    (target / "llvm-cov").mkdir()
    (target / "debug").mkdir()
    monkeypatch.setenv("INPUT_TARGET_DIR", str(target))
    return target


def _df(cmd_mox: CmdMox, *, exit_code: int = 0, expected_calls: int = 2) -> None:
    """Mock the ``df -h .`` free-space pre-flight the script runs twice.

    ``expected_calls`` is what cmd-mox verifies at teardown, so a case whose
    run stops between the two calls passes ``1``: the count is an assertion
    about how far the script got, not a fixture detail.
    """
    cmd_mox.mock("df").with_args("-h", ".").returns(
        stdout="Filesystem 1%\n", exit_code=exit_code
    ).times(expected_calls)


def test_removes_the_instrumented_trees_and_keeps_the_rest(
    cmd_mox: CmdMox, target: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Both llvm-cov trees go; the ordinary debug tree stays; df runs twice."""
    _df(cmd_mox)

    exit_code = invoke(script.app)

    assert exit_code == 0, "expected: exit_code == 0"
    assert not (target / "llvm-cov-target").exists(), (
        "expected: not (target / 'llvm-cov-target').exists()"
    )
    assert not (target / "llvm-cov").exists(), (
        "expected: not (target / 'llvm-cov').exists()"
    )
    assert (target / "debug").is_dir(), "expected: (target / 'debug').is_dir()"
    out = capsys.readouterr().out
    assert f"removed {target / 'llvm-cov-target'}" in out, (
        "expected: f'removed {target / 'llvm-cov-target'}' in out"
    )


def test_missing_trees_are_not_an_error(
    cmd_mox: CmdMox, monkeypatch: pytest.MonkeyPatch, tmp_path: pathlib.Path
) -> None:
    """A lane that never built coverage has nothing to discard and succeeds."""
    monkeypatch.setenv("INPUT_TARGET_DIR", str(tmp_path / "absent"))
    _df(cmd_mox)

    assert invoke(script.app) == 0, "expected: invoke(script.app) == 0"


def test_symlinked_tree_is_unlinked_not_followed(
    cmd_mox: CmdMox, target: pathlib.Path, tmp_path: pathlib.Path
) -> None:
    """A link in place of a tree is removed as a link; its target survives."""
    elsewhere = tmp_path / "elsewhere"
    elsewhere.mkdir()
    (elsewhere / "keep").write_text("keep", encoding="utf-8")
    link = target / "llvm-cov"
    link.rmdir()
    link.symlink_to(elsewhere)
    _df(cmd_mox)

    assert invoke(script.app) == 0, "expected: invoke(script.app) == 0"

    assert not link.exists(), "expected: not link.exists()"
    assert (elsewhere / "keep").exists(), "expected: (elsewhere / 'keep').exists()"


def test_subtrees_input_overrides_the_default_names(
    cmd_mox: CmdMox, target: pathlib.Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """INPUT_SUBTREES selects which names are removed, whitespace separated."""
    monkeypatch.setenv("INPUT_SUBTREES", "debug llvm-cov")
    _df(cmd_mox)

    assert invoke(script.app) == 0, "expected: invoke(script.app) == 0"

    assert not (target / "debug").exists(), "expected: not (target / 'debug').exists()"
    assert not (target / "llvm-cov").exists(), (
        "expected: not (target / 'llvm-cov').exists()"
    )
    assert (target / "llvm-cov-target").is_dir(), (
        "expected: (target / 'llvm-cov-target').is_dir()"
    )


def test_a_bad_name_removes_nothing_at_all(
    cmd_mox: CmdMox,
    target: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A bad name fails the step before the first tree is removed.

    The name after the bad one is valid, so a removal loop that validated as
    it went would delete ``llvm-cov`` and then raise: the step would fail, but
    a tree the caller never agreed to lose would already be gone. Every name
    is resolved before the first removal so the failure is total.
    """
    monkeypatch.setenv("INPUT_SUBTREES", "llvm-cov ..")
    # The run stops before the post-removal df, so only the pre-flight is made.
    _df(cmd_mox, expected_calls=1)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    assert "refusing to remove '..'" in capsys.readouterr().err, (
        "expected: the diagnostic to name the refused component"
    )
    assert (target / "llvm-cov").is_dir(), (
        "the valid name ahead of the bad one must survive"
    )
    assert (target / "llvm-cov-target").is_dir(), (
        "expected: (target / 'llvm-cov-target').is_dir()"
    )


def test_a_failing_post_removal_df_fails_the_step_after_removing(
    cmd_mox: CmdMox, target: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """The second ``df`` failing fails the step, with the removal already done.

    The pre-flight ``df`` has its own case. This is the other call, and it
    reports what the removal freed -- the number a lane reads to decide whether
    the cache save will fit. A step that swallowed this failure would report
    success while leaving that number unrecorded, so the removal stands and the
    step still fails.

    One mock answers both calls and fails only the second, because the two are
    distinguished by when they happen rather than by what they ask for: both
    run ``df -h .``, so nothing in the arguments could tell two separate mocks
    apart.
    """
    calls: list[list[str]] = []

    def handle(invocation: Invocation) -> tuple[str, str, int]:
        """Succeed the pre-flight ``df`` and fail the post-removal one."""
        calls.append(list(invocation.args))
        if len(calls) == 1:
            return ("Filesystem 1%\n", "", 0)
        return ("", "df: cannot read\n", 1)

    cmd_mox.mock("df").runs(handle).times(2)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    assert len(calls) == 2, "both disk-usage calls must have been made"
    captured = capsys.readouterr()
    assert "reading disk usage failed" in captured.err, (
        "expected: 'reading disk usage failed' in captured.err"
    )
    assert not (target / "llvm-cov-target").exists(), (
        "the removal itself succeeded and must not be undone"
    )
    assert not (target / "llvm-cov").exists(), (
        "expected: not (target / 'llvm-cov').exists()"
    )
    assert "removed " not in captured.out, (
        "the per-tree report is skipped when the step fails"
    )


def test_a_failing_df_fails_the_step_before_removing_anything(
    cmd_mox: CmdMox, target: pathlib.Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Disk usage must be observable; if it is not, nothing is discarded."""
    cmd_mox.mock("df").with_args("-h", ".").returns(exit_code=1)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    assert "reading disk usage failed" in capsys.readouterr().err, (
        "expected: 'reading disk usage failed' in capsys.readouterr().err"
    )
    assert (target / "llvm-cov-target").is_dir(), (
        "expected: (target / 'llvm-cov-target').is_dir()"
    )
