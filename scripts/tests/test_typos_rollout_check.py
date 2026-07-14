"""Test exact phrase-policy enforcement."""

import importlib
from pathlib import Path
import subprocess
import types

import pytest


SCRIPTS = Path(__file__).resolve().parents[1]
PROHIBITED = "hand" + "-written"
TITLE_PROHIBITED = "Hand" + "-written"
SECOND_PROHIBITED = "spell" + "-checked"


@pytest.fixture(name="modules")
def modules_fixture(
    monkeypatch: pytest.MonkeyPatch,
) -> tuple[types.ModuleType, types.ModuleType]:
    """Load the rollout and phrase-checker modules from the scripts directory."""
    monkeypatch.syspath_prepend(str(SCRIPTS))
    importlib.invalidate_caches()
    return (
        importlib.import_module("typos_rollout"),
        importlib.import_module("typos_rollout_check"),
    )


def initialize(path: Path, files: dict[str, str]) -> None:
    """Create and stage a minimal repository containing the supplied files."""
    for relative, content in files.items():
        target = path / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content, encoding="utf-8")
    subprocess.run(["git", "init", "--quiet"], cwd=path, check=True)
    subprocess.run(["git", "add", "."], cwd=path, check=True)


def test_checker_boundaries_ignores_exclusions(
    modules: tuple[types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """The checker honours token boundaries, ignored spans, and exclusions."""
    rollout, check = modules
    initialize(
        tmp_path,
        {
            "README.md": (f"{PROHIBITED}\n{TITLE_PROHIBITED} prose\n`{PROHIBITED}`\n"),
            "skip.md": f"{PROHIBITED}\n",
            "joined.md": "pre-hand" + "-written\n",
        },
    )
    policy = rollout.Dictionary(
        phrase_corrections=((PROHIBITED, "handwritten"),),
        ignore_patterns=(r"`[^`\n]+`",),
        excluded_files=("skip.md",),
    )

    actual = [
        (finding.line, finding.phrase)
        for finding in check.check_phrase_corrections(tmp_path, policy)
    ]
    expected = [(1, PROHIBITED), (2, TITLE_PROHIBITED)]

    assert actual == expected, "phrase boundaries or policy exclusions changed"


def test_checker_orders_complete_findings_by_path_phrase_and_source(
    modules: tuple[types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """Findings retain deterministic order, positions, case and corrections."""
    rollout, check = modules
    initialize(
        tmp_path,
        {
            "b.md": f"{SECOND_PROHIBITED} then {PROHIBITED}\n",
            "a.md": (f"{PROHIBITED} then {SECOND_PROHIBITED} and {TITLE_PROHIBITED}\n"),
        },
    )
    policy = rollout.Dictionary(
        phrase_corrections=(
            (PROHIBITED, "handwritten"),
            (SECOND_PROHIBITED, "spellchecked"),
        ),
    )

    actual = check.check_phrase_corrections(tmp_path, policy)
    expected = (
        check.PhraseFinding(Path("a.md"), 1, 1, PROHIBITED, "handwritten"),
        check.PhraseFinding(Path("a.md"), 1, 37, TITLE_PROHIBITED, "handwritten"),
        check.PhraseFinding(Path("a.md"), 1, 19, SECOND_PROHIBITED, "spellchecked"),
        check.PhraseFinding(Path("b.md"), 1, 20, PROHIBITED, "handwritten"),
        check.PhraseFinding(Path("b.md"), 1, 1, SECOND_PROHIBITED, "spellchecked"),
    )

    assert actual == expected, "finding order or diagnostic content changed"


def test_main_reports(
    modules: tuple[types.ModuleType, types.ModuleType],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """The command reports source positions and returns a failure status."""
    rollout, check = modules
    initialize(tmp_path, {"README.md": f"Prefer {PROHIBITED}.\n"})
    monkeypatch.setattr(
        check.generator,
        "dictionary_from_cache",
        lambda _: rollout.Dictionary(phrase_corrections=((PROHIBITED, "handwritten"),)),
    )

    status = check.main(["--repository", str(tmp_path)])
    output = capsys.readouterr().out

    assert status == 2, "the command accepted a prohibited phrase"
    assert "README.md:1:8:" in output, "the diagnostic omitted its source position"
