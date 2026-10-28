"""Exercise malformed staging configurations at the declaration CLI boundary."""

import typing as typ

import pytest
from release_declaration_cli import EXIT_INPUT_UNUSABLE, main

if typ.TYPE_CHECKING:
    from pathlib import Path


@pytest.mark.parametrize("config", ['targets = "bad"\n', '[targets]\nlinux = "bad"\n'])
def test_malformed_targets_are_reported_as_input_faults(
    tmp_path: Path, config: str, capsys: pytest.CaptureFixture[str]
) -> None:
    """Reject malformed target tables without a traceback or output."""
    staging = tmp_path / "staging.toml"
    _ = staging.write_text(config, encoding="utf-8")
    output = tmp_path / "names"
    record = tmp_path / "record"
    code = main([
        "--dist-dir",
        str(tmp_path),
        "--staging-config",
        str(staging),
        "--manifest",
        str(tmp_path / "manifest"),
        "--version",
        "1.0.0",
        "--output",
        str(output),
        "--digest-record",
        str(record),
    ])
    assert code == EXIT_INPUT_UNUSABLE, f"malformed targets returned {code}"
    assert "inputs unusable" in capsys.readouterr().err, "missing input diagnostic"
    assert not output.exists(), "malformed targets wrote a declaration"
    assert not record.exists(), "malformed targets wrote a digest record"
