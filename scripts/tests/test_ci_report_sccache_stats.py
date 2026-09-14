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


def _sccache(cmd_mox: CmdMox, *, json_exit: int = 0) -> list[list[str]]:
    """Mock ``sccache`` for the text and JSON reports, recording its calls."""
    calls: list[list[str]] = []

    def handle(invocation: Invocation) -> tuple[str, str, int]:
        calls.append(list(invocation.args))
        if "--stats-format=json" in invocation.args:
            return (JSON_STATS, "", json_exit)
        return (TEXT_STATS, "", 0)

    cmd_mox.mock("sccache").runs(handle).times(2)
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


def test_summary_section_terminates_unterminated_text() -> None:
    """The fenced block always closes on its own line."""
    section = script.summary_section("hits 1")

    assert section.startswith("### sccache\n\n```text\n"), section
    assert section.endswith("hits 1\n```\n"), "the fence must close on its own line"
