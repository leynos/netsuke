"""Test GLIBC floor reporting against the checked-in readelf fixtures."""

import pathlib
import typing as typ

import pytest
import report_glibc_floor as reporter
from hypothesis import given
from hypothesis import strategies as st

if typ.TYPE_CHECKING:
    from cmd_mox import CmdMox


class ReportContext(typ.NamedTuple):
    """Collect the pytest fixtures used by GLIBC reporting checks."""

    cmd_mox: CmdMox
    monkeypatch: pytest.MonkeyPatch
    tmp_path: pathlib.Path
    capsys: pytest.CaptureFixture[str]


@pytest.fixture
def report_context(
    cmd_mox: CmdMox,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: pathlib.Path,
    capsys: pytest.CaptureFixture[str],
) -> ReportContext:
    """Provide the command, environment, file and output test seams."""
    return ReportContext(cmd_mox, monkeypatch, tmp_path, capsys)


pytest_plugins = ("cmd_mox.pytest_plugin",)

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
READELF_FIXTURES = REPO_ROOT / "tests" / "data"


@given(
    versions=st.lists(
        st.lists(st.integers(min_value=0, max_value=1000), min_size=1, max_size=5),
        min_size=1,
        max_size=12,
    )
)
def test_highest_glibc_version_uses_numeric_order_within_needs_section(
    versions: list[list[int]],
) -> None:
    """Select the numeric maximum and ignore a higher version in a later section."""
    highest = ".".join(map(str, max(versions)))
    rendered = [".".join(map(str, version)) for version in versions]
    irrelevant = f"{max(version[0] for version in versions) + 1}.0"
    output = "Version needs section x\n"
    output += "\n".join(f"GLIBC_{version}" for version in rendered)
    output += f"\nVersion symbols section x\nGLIBC_{irrelevant}\n"

    assert reporter._highest_glibc_version(output) == f"GLIBC_{highest}", (
        "the floor must be the highest numeric requirement from the needs section"
    )


@pytest.mark.parametrize(
    ("fixture", "target", "floor"),
    [
        pytest.param(
            "readelf-version-info.txt",
            "x86_64-unknown-linux-gnu",
            "GLIBC_2.34",
            id="native",
        ),
        pytest.param(
            "readelf-version-info-aarch64.txt",
            "aarch64-unknown-linux-gnu",
            "GLIBC_2.18",
            id="cross",
        ),
    ],
)
def test_report_uses_the_highest_required_version(
    report_context: ReportContext,
    fixture: str,
    target: str,
    floor: str,
) -> None:
    """Ignore symbol and definition sections when reporting the floor."""
    cmd_mox = report_context.cmd_mox
    monkeypatch = report_context.monkeypatch
    tmp_path = report_context.tmp_path
    output = (READELF_FIXTURES / fixture).read_text(encoding="utf-8")
    binary = f"target/{target}/release/netsuke"
    cmd_mox.mock("readelf").with_args("--version-info", binary).returns(
        exit_code=0, stdout=output
    )
    summary_path = tmp_path / "summary.md"
    monkeypatch.setenv("INPUT_TARGET", target)
    monkeypatch.setenv("INPUT_BIN_NAME", "netsuke")
    monkeypatch.setenv("GITHUB_STEP_SUMMARY", str(summary_path))

    reporter.app([], result_action="return_value")

    captured = report_context.capsys.readouterr()
    assert captured.out == f"glibc floor for {target}: {floor}\n", (
        "the script must print the greatest version from the needs section"
    )
    assert summary_path.read_text(encoding="utf-8") == (
        f"- glibc floor for `{target}`: `{floor}`\n"
    ), "the workflow summary must record the same GLIBC floor"


def test_readelf_failure_preserves_status_and_stderr(
    report_context: ReportContext,
) -> None:
    """A readelf error is reported and prevents a misleading summary entry."""
    cmd_mox = report_context.cmd_mox
    monkeypatch = report_context.monkeypatch
    tmp_path = report_context.tmp_path
    binary = "target/x86_64-unknown-linux-gnu/release/netsuke"
    cmd_mox.mock("readelf").with_args("--version-info", binary).returns(
        exit_code=17, stderr="not an ELF file\n"
    )
    summary_path = tmp_path / "summary.md"
    monkeypatch.setenv("INPUT_TARGET", "x86_64-unknown-linux-gnu")
    monkeypatch.setenv("INPUT_BIN_NAME", "netsuke")
    monkeypatch.setenv("GITHUB_STEP_SUMMARY", str(summary_path))

    with pytest.raises(SystemExit) as error:
        reporter.app([])

    assert error.value.code == 17, "the script must preserve readelf's exit status"
    assert report_context.capsys.readouterr().err == "not an ELF file\n", (
        "readelf's diagnostic must reach the workflow log"
    )
    assert not summary_path.exists(), "a failed readelf call must not report a floor"


def test_missing_glibc_version_preserves_pipeline_failure(
    report_context: ReportContext,
) -> None:
    """A successful readelf without GLIBC requirements still fails the step."""
    report_context.cmd_mox.mock("readelf").with_args(
        "--version-info", "target/x86_64-unknown-linux-gnu/release/netsuke"
    ).returns(
        exit_code=0, stdout="Version needs section x\nVersion symbols section y\n"
    )
    summary_path = report_context.tmp_path / "summary.md"
    report_context.monkeypatch.setenv("INPUT_TARGET", "x86_64-unknown-linux-gnu")
    report_context.monkeypatch.setenv("INPUT_BIN_NAME", "netsuke")
    report_context.monkeypatch.setenv("GITHUB_STEP_SUMMARY", str(summary_path))

    with pytest.raises(SystemExit) as error:
        reporter.app([])

    assert error.value.code == 1, (
        "the old pipefail pipeline fails when grep finds no version"
    )
    assert report_context.capsys.readouterr().err == (
        "no GLIBC version requirements found for "
        "target/x86_64-unknown-linux-gnu/release/netsuke\n"
    ), "a missing floor must identify the binary in the workflow log"
    assert not summary_path.exists(), "a missing GLIBC floor must not enter the summary"
