"""Exercise the Markdown formatter checker at its process boundary."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import textwrap
from pathlib import Path

import pytest
from hypothesis import given, settings, strategies as st

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
CHECKER = REPOSITORY_ROOT / "scripts" / "check-markdown-format.sh"
FORMATTER_FLAGS = [
    "--in-place",
    "--wrap",
    "--renumber",
    "--breaks",
    "--ellipsis",
    "--fences",
]
LINE_ALPHABET = tuple("abcdefghijklmnopqrstuvwxyz0123456789 -_[]*")


def _write_fake_formatter(directory: Path) -> tuple[Path, Path]:
    """Create a formatter fixture that records calls and canonicalises bytes."""
    executable = directory / "mdtablefix"
    call_log = directory / "formatter-calls.jsonl"
    executable.write_text(
        textwrap.dedent(
            """\
            #!__PYTHON__
            import json
            import os
            import pathlib
            import sys

            arguments = sys.argv[1:]
            if arguments == ["--version"]:
                print("mdtablefix 0.5.0")
                raise SystemExit(0)

            expected_flags = [
                "--in-place",
                "--wrap",
                "--renumber",
                "--breaks",
                "--ellipsis",
                "--fences",
            ]
            if arguments[:len(expected_flags)] != expected_flags:
                print("unexpected formatter flags", file=sys.stderr)
                raise SystemExit(64)

            paths = arguments[len(expected_flags):]
            if not paths:
                print("expected at least one Markdown path", file=sys.stderr)
                raise SystemExit(64)

            with pathlib.Path(os.environ["MDTABLEFIX_CALL_LOG"]).open("a") as log:
                print(json.dumps(arguments), file=log)

            for path in paths:
                source = pathlib.Path(path)
                canonical = source.read_bytes().replace(b"\\r\\n", b"\\n")
                canonical = canonical.replace(b"\\r", b"\\n")
                source.write_bytes(canonical.replace(b"unformatted", b"formatted"))
            """
        ).replace("__PYTHON__", sys.executable),
        encoding="utf-8",
    )
    executable.chmod(0o755)
    return executable, call_log


@pytest.fixture
def formatter(tmp_path: Path) -> tuple[Path, Path]:
    """Provide a controlled formatter executable and its call log."""
    return _write_fake_formatter(tmp_path)


def _run_checker(
    formatter: Path,
    call_log: Path,
    *files: Path,
) -> subprocess.CompletedProcess[str]:
    """Run the checker against files with the controlled formatter fixture."""
    environment = os.environ | {
        "MDTABLEFIX": str(formatter),
        "MDTABLEFIX_CALL_LOG": str(call_log),
    }
    return subprocess.run(  # noqa: S603 - executes the controlled fixture.
        [str(CHECKER), *(str(file) for file in files)],
        capture_output=True,
        check=False,
        env=environment,
        text=True,
    )


def test_requires_at_least_one_markdown_file(formatter: tuple[Path, Path]) -> None:
    """Reject an empty file list before trying to invoke the formatter."""
    executable, call_log = formatter

    result = _run_checker(executable, call_log)

    assert result.returncode == 64
    assert "Usage:" in result.stderr
    assert not call_log.exists()


def test_reports_a_missing_formatter(tmp_path: Path) -> None:
    """Explain how to supply the formatter when its executable is unavailable."""
    source = tmp_path / "source.md"
    source.write_text("formatted\n", encoding="utf-8")

    result = _run_checker(tmp_path / "missing-mdtablefix", tmp_path / "calls", source)

    assert result.returncode == 127
    assert "is not installed or not on PATH" in result.stderr


def test_accepts_lf_and_crlf_without_modifying_sources(
    formatter: tuple[Path, Path],
    tmp_path: Path,
) -> None:
    """Accept exact canonical output in either Git checkout line-ending form."""
    executable, call_log = formatter
    lf_source = tmp_path / "lf.md"
    crlf_source = tmp_path / "crlf.md"
    lf_source.write_bytes(b"formatted\n")
    crlf_source.write_bytes(b"formatted\r\n")

    result = _run_checker(executable, call_log, lf_source, crlf_source)

    assert result.returncode == 0
    assert lf_source.read_bytes() == b"formatted\n"
    assert crlf_source.read_bytes() == b"formatted\r\n"
    calls = [
        json.loads(line) for line in call_log.read_text(encoding="utf-8").splitlines()
    ]
    assert len(calls) == 1
    assert calls[0][: len(FORMATTER_FLAGS)] == FORMATTER_FLAGS
    staged_paths = [Path(path) for path in calls[0][len(FORMATTER_FLAGS) :]]
    assert [path.name for path in staged_paths] == ["0.md", "1.md"]
    assert all(path.parent != tmp_path for path in staged_paths)


def test_reports_each_noncanonical_source_without_modifying_it(
    formatter: tuple[Path, Path],
    tmp_path: Path,
) -> None:
    """Reject altered and mixed-ending files while leaving every source intact."""
    executable, call_log = formatter
    unformatted_source = tmp_path / "unformatted.md"
    mixed_source = tmp_path / "mixed.md"
    canonical_source = tmp_path / "canonical.md"
    unformatted_source.write_bytes(b"unformatted\n")
    mixed_source.write_bytes(b"formatted\r\nsecond line\n")
    canonical_source.write_bytes(b"formatted\n")

    result = _run_checker(
        executable,
        call_log,
        unformatted_source,
        mixed_source,
        canonical_source,
    )

    assert result.returncode == 1
    assert str(unformatted_source) in result.stderr
    assert str(mixed_source) in result.stderr
    assert str(canonical_source) not in result.stderr
    assert unformatted_source.read_bytes() == b"unformatted\n"
    assert mixed_source.read_bytes() == b"formatted\r\nsecond line\n"
    assert canonical_source.read_bytes() == b"formatted\n"
    calls = [
        json.loads(line) for line in call_log.read_text(encoding="utf-8").splitlines()
    ]
    assert len(calls) == 1
    assert len(calls[0]) == len(FORMATTER_FLAGS) + 3


@given(
    lines=st.lists(st.text(alphabet=LINE_ALPHABET, max_size=40), min_size=2, max_size=5)
)
@settings(deadline=None)
def test_accepts_only_uniform_canonical_line_endings(lines: list[str]) -> None:
    """Accept LF and CRLF canonical forms while rejecting a mixed equivalent."""
    canonical = "\n".join(lines) + "\n"
    with tempfile.TemporaryDirectory() as directory_name:
        directory = Path(directory_name)
        executable, call_log = _write_fake_formatter(directory)
        lf_source = directory / "lf.md"
        crlf_source = directory / "crlf.md"
        mixed_source = directory / "mixed.md"
        lf_source.write_text(canonical, encoding="utf-8")
        crlf_source.write_bytes(canonical.replace("\n", "\r\n").encode())
        mixed_source.write_bytes(
            (lines[0] + "\r\n" + "\n".join(lines[1:]) + "\n").encode()
        )

        passing = _run_checker(executable, call_log, lf_source, crlf_source)
        failing = _run_checker(executable, call_log, mixed_source)

    assert passing.returncode == 0
    assert failing.returncode == 1
    assert str(mixed_source) in failing.stderr
