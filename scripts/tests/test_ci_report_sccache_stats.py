"""Behavioural tests for ``scripts/ci/report_sccache_stats.py``."""

import typing as typ

import pytest
from ci_script_support import invoke, load_ci_script

if typ.TYPE_CHECKING:
    import pathlib

    from cmd_mox import CmdMox
    from cmd_mox.ipc import Invocation

script = load_ci_script("report_sccache_stats")

TEXT_STATS = "Compile requests    12\nCache hits          10\n"
JSON_STATS = '{"stats":{"compile_requests":12}}\n'


@pytest.fixture
def summary(monkeypatch: pytest.MonkeyPatch, tmp_path: pathlib.Path) -> pathlib.Path:
    """Provide GITHUB_STEP_SUMMARY with prior content and the output directory."""
    summary = tmp_path / "summary.md"
    summary.write_text("## Earlier\n", encoding="utf-8")
    monkeypatch.setenv("GITHUB_STEP_SUMMARY", str(summary))
    monkeypatch.setenv("INPUT_OUTPUT_DIR", str(tmp_path))
    return summary


def _sccache(
    cmd_mox: CmdMox,
    *,
    text_exit: int = 0,
    json_exit: int = 0,
    expected_calls: int = 2,
) -> list[list[str]]:
    """Mock ``sccache`` for the text and JSON reports, recording its calls.

    The two reports fail independently, so a test can prove that a text-report
    failure is not masked by the JSON report that would follow it, and vice
    versa. ``expected_calls`` is what cmd-mox verifies at teardown, so a
    failure that stops the script early passes ``1``.

    Returns
    -------
    list[list[str]]
        The argument lists recorded so far, which the live mock appends to as
        the script runs.
    """
    calls: list[list[str]] = []

    def handle(invocation: Invocation) -> tuple[str, str, int]:
        """Record the call and fail the report the test selected."""
        calls.append(list(invocation.args))
        if "--stats-format=json" in invocation.args:
            return (JSON_STATS, "", json_exit)
        return (TEXT_STATS, "", text_exit)

    cmd_mox.mock("sccache").runs(handle).times(expected_calls)
    return calls


def test_writes_text_json_and_the_summary_section(
    cmd_mox: CmdMox, summary: pathlib.Path, tmp_path: pathlib.Path
) -> None:
    """Both report files land in the output dir and the summary gains a block."""
    calls = _sccache(cmd_mox)

    exit_code = invoke(script.app)

    assert exit_code == 0, "expected: exit_code == 0"
    assert calls == [["--show-stats"], ["--show-stats", "--stats-format=json"]], (
        "expected: calls == [['--show-stats'], ['--show-stats', '--stats-for..."
    )
    assert (tmp_path / "sccache-stats.txt").read_text(encoding="utf-8") == TEXT_STATS, (
        "expected: (tmp_path / 'sccache-stats.txt').read_text(encoding='utf-..."
    )
    assert (tmp_path / "sccache-stats.json").read_text(
        encoding="utf-8"
    ) == JSON_STATS, (
        "expected: (tmp_path / 'sccache-stats.json').read_text(encoding='utf..."
    )
    assert summary.read_text(encoding="utf-8") == (
        "## Earlier\n### sccache\n\n```text\n" + TEXT_STATS + "```\n"
    ), "expected: summary.read_text(encoding='utf-8') == ( '## Earlier\n###..."


def test_a_failing_report_leaves_no_files_and_fails_the_step(
    cmd_mox: CmdMox,
    summary: pathlib.Path,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A non-zero sccache exit is surfaced and nothing partial is written."""
    _sccache(cmd_mox, json_exit=2)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    assert "reading statistics failed" in capsys.readouterr().err, (
        "expected: 'reading statistics failed' in capsys.readouterr().err"
    )
    assert not (tmp_path / "sccache-stats.txt").exists(), (
        "expected: not (tmp_path / 'sccache-stats.txt').exists()"
    )
    assert not (tmp_path / "sccache-stats.json").exists(), (
        "expected: not (tmp_path / 'sccache-stats.json').exists()"
    )
    assert summary.read_text(encoding="utf-8") == "## Earlier\n", (
        "expected: summary.read_text(encoding='utf-8') == '## Earlier\n'"
    )


def test_a_failing_text_report_leaves_no_files_and_fails_the_step(
    cmd_mox: CmdMox,
    summary: pathlib.Path,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """A non-zero text report stops the step before the JSON report is asked for.

    The text report is the first call and the one the summary is built from, so
    a failure there is the case where a partial write is most tempting: the
    JSON report would still succeed, and the summary would still have something
    to show. Failing the JSON report instead would not cover it.
    """
    calls = _sccache(cmd_mox, text_exit=2, expected_calls=1)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    assert calls == [["--show-stats"]], (
        "a failed text report must stop the step before the JSON report runs"
    )
    assert "reading statistics failed" in capsys.readouterr().err, (
        "expected: 'reading statistics failed' in capsys.readouterr().err"
    )
    assert not (tmp_path / "sccache-stats.txt").exists(), (
        "expected: not (tmp_path / 'sccache-stats.txt').exists()"
    )
    assert not (tmp_path / "sccache-stats.json").exists(), (
        "expected: not (tmp_path / 'sccache-stats.json').exists()"
    )
    assert summary.read_text(encoding="utf-8") == "## Earlier\n", (
        "expected: summary.read_text(encoding='utf-8') == '## Earlier\n'"
    )


def test_an_unwritable_summary_fails_the_step_after_the_files_are_written(
    cmd_mox: CmdMox,
    summary: pathlib.Path,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """An unwritable step summary is a diagnostic, not a traceback.

    The summary is written last, so the two report files are already on disk
    when this fails. That is deliberate: they are what the workflow uploads as
    an artifact, and losing them because the summary could not be appended
    would discard the evidence this step exists to collect. The step still
    fails, because a lane that silently reported nothing to the summary would
    hide a broken report for as long as it stayed broken.
    """
    summary.chmod(0o444)
    # Probe the precondition rather than assume it: the mode bits are what this
    # test needs, and root -- or a filesystem that does not enforce them --
    # would let the append succeed, so the step would exit 0 and the assertions
    # below would fail for a reason that has nothing to do with the script.
    # `os.access` cannot stand in here, because it reports no write permission
    # for root while root can still write.
    try:
        with summary.open("a", encoding="utf-8"):
            pass
    except PermissionError:
        pass
    else:
        pytest.skip("file permissions are not enforced for this user")
    _sccache(cmd_mox)

    exit_code = invoke(script.app)

    assert exit_code == 1, "expected: exit_code == 1"
    err = capsys.readouterr().err
    assert "report_sccache_stats:" in err, (
        "expected: the failure to be reported as a diagnostic"
    )
    assert "Traceback" not in err, "the step must not fail with a traceback"
    assert (tmp_path / "sccache-stats.txt").read_text(encoding="utf-8") == TEXT_STATS, (
        "expected: the text report is kept even when the summary cannot be written"
    )
    assert summary.read_text(encoding="utf-8") == "## Earlier\n", (
        "expected: the summary is unchanged"
    )


def test_summary_section_terminates_unterminated_text() -> None:
    """The fenced block always closes on its own line."""
    section = script.summary_section("hits 1")

    assert section.startswith("### sccache\n\n```text\n"), section
    assert section.endswith("hits 1\n```\n"), "the fence must close on its own line"


def test_names_the_backend_the_lane_selected(
    cmd_mox: CmdMox,
    summary: pathlib.Path,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """`SCCACHE_BACKEND` puts a backend line above the statistics.

    The statistics' own ``Cache location`` cannot stand in for it: that reads
    ``ghac`` for the Ubicloud cache proxy and for GitHub's own service alike.
    """
    monkeypatch.setenv("SCCACHE_BACKEND", "ubicloud")
    _sccache(cmd_mox)

    exit_code = invoke(script.app)

    assert exit_code == 0, "expected: exit_code == 0"
    assert summary.read_text(encoding="utf-8") == (
        "## Earlier\n### sccache\n\n- backend: ubicloud\n\n```text\n"
        + TEXT_STATS
        + "```\n"
    ), "the backend line must precede the statistics block"


def test_a_lane_without_a_backend_gets_no_backend_line(
    cmd_mox: CmdMox, summary: pathlib.Path
) -> None:
    """A lane that wires its own cache reports statistics and no backend.

    `SCCACHE_BACKEND` is unset there, so the summary keeps the shape those
    lanes had before the script took the report over.
    """
    _sccache(cmd_mox)

    exit_code = invoke(script.app)

    assert exit_code == 0, "expected: exit_code == 0"
    assert "- backend:" not in summary.read_text(encoding="utf-8"), (
        "an empty backend must add no line, not an empty one"
    )
