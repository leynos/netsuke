"""Exercise malformed staging configurations at the declaration CLI boundary."""

import typing as typ

import pytest
from release_declaration_cli import EXIT_INPUT_UNUSABLE, main

if typ.TYPE_CHECKING:
    from pathlib import Path


class Case(typ.NamedTuple):
    """One malformed staging configuration and the diagnosis it must produce."""

    config: str
    diagnosis: str
    reason: str


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(
            Case(
                'targets = "bad"\n',
                "no release targets",
                "the targets member is not a table",
            ),
            id="targets-not-a-table",
        ),
        pytest.param(
            Case(
                "targets = []\n", "no release targets", "a list is not a target table"
            ),
            id="targets-is-a-list",
        ),
        pytest.param(
            Case(
                "targets = {}\n",
                "no release targets",
                "an empty table declares no release target",
            ),
            id="targets-empty",
        ),
        pytest.param(
            Case(
                "common = {}\n",
                "no release targets",
                "an absent targets table declares no release target",
            ),
            id="targets-absent",
        ),
        pytest.param(
            Case(
                '[targets]\nlinux = "bad"\n',
                "target without a platform",
                "a target entry is not a table",
            ),
            id="entry-is-a-string",
        ),
        pytest.param(
            Case(
                "[targets]\nlinux = 1\n",
                "target without a platform",
                "a target entry is not a table",
            ),
            id="entry-is-an-integer",
        ),
        pytest.param(
            Case(
                "[targets]\nlinux = {}\n",
                "target without a platform",
                "a target table has no platform",
            ),
            id="entry-has-no-platform",
        ),
        pytest.param(
            Case(
                "[targets]\nlinux = { platform = 1 }\n",
                "target without a platform",
                "a platform must be a string",
            ),
            id="platform-is-an-integer",
        ),
    ],
)
def test_malformed_targets_are_reported_as_input_faults(
    tmp_path: Path, case: Case, capsys: pytest.CaptureFixture[str]
) -> None:
    """Reject malformed target tables without a traceback or output.

    Every configuration error exits with the same code, so the diagnosis is
    what distinguishes a missing targets table from an unusable target entry.
    Asserting it keeps each preserved rejection category under test.
    """
    staging = tmp_path / "staging.toml"
    _ = staging.write_text(case.config, encoding="utf-8")
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
    assert code == EXIT_INPUT_UNUSABLE, (
        f"{case.reason}: malformed targets returned {code}"
    )
    stderr = capsys.readouterr().err
    assert "inputs unusable" in stderr, "missing input diagnostic"
    assert case.diagnosis in stderr, f"{case.reason}: wrong input diagnostic"
    assert not output.exists(), "malformed targets wrote a declaration"
    assert not record.exists(), "malformed targets wrote a digest record"
