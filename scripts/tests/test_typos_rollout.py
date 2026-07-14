"""Tests for the repository spelling-policy scripts."""

from __future__ import annotations

import ast
import tomllib
import typing as typ
from pathlib import Path

import pytest

from typos_rollout_test_support import dictionary_text as _dictionary_text

if typ.TYPE_CHECKING:
    import types

SCRIPT_DIRECTORY = Path(__file__).resolve().parents[1]


def test_rollout_scripts_support_python_313() -> None:
    """Every rollout script parses with the declared minimum Python version."""
    for script in SCRIPT_DIRECTORY.glob("*.py"):
        ast.parse(
            script.read_text(encoding="utf-8"),
            filename=str(script),
            feature_version=(3, 13),
        )


def test_rollout_generates_oxford_corrections(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
) -> None:
    """The shared renderer accepts Oxford forms and corrects plain-British ones."""
    _, rollout, _ = rollout_modules

    mappings = rollout.generate_word_mappings(rollout.Dictionary(stems=("organ",)))

    assert mappings["organize"] == "organize", "Oxford form was not accepted"
    assert mappings["organise"] == "organize", "plain-British form was not fixed"
    italic_mappings = rollout.generate_word_mappings(
        rollout.Dictionary(stems=("italic",))
    )
    assert italic_mappings["italicised"] == "italicized", (
        "plain-British italic form was not fixed"
    )


def test_https_failure_reuses_valid_tracked_config(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """A clean network-restricted checkout retains its reviewed policy."""
    _, rollout, generator = rollout_modules
    tracked_config = tmp_path / "typos.toml"
    tracked_config.write_text('[default]\nlocale = "en-gb"\n', encoding="utf-8")

    def unavailable(*_args: object, **_kwargs: object) -> None:
        """Model an unavailable HTTPS authority."""
        raise rollout.NetworkUnavailableError("offline")

    monkeypatch.setattr(rollout, "refresh_base", unavailable)

    result = generator.main(repository=tmp_path, source="https://example.invalid/base")

    assert result.status == "tracked-config", "tracked fallback was not selected"
    assert result.cache == tracked_config, "fallback did not return tracked config"


@pytest.mark.parametrize(
    "document",
    [
        pytest.param(
            _dictionary_text().replace("schema = 1", "schema = 2"),
            id="unsupported-schema",
        ),
        pytest.param(
            _dictionary_text().replace(
                '[oxford]\nstems = ["organ"]',
                'oxford = "bad"',
            ),
            id="invalid-table",
        ),
        pytest.param(
            _dictionary_text().replace('stems = ["organ"]', "stems = [1]"),
            id="invalid-string-list",
        ),
        pytest.param(
            _dictionary_text().replace(
                "[words.corrections]",
                "[words.corrections]\nteh = 1",
            ),
            id="invalid-correction",
        ),
    ],
)
def test_dictionary_validation_rejects_invalid_documents(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
    document: str,
) -> None:
    """Schema, table, string-list and correction types remain validated."""
    _, rollout, _ = rollout_modules
    source = tmp_path / "base.toml"
    source.write_text(document, encoding="utf-8")

    with pytest.raises((TypeError, ValueError)):
        rollout.load_dictionary(source)


def test_merge_rejects_conflicting_corrections(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
) -> None:
    """A local overlay cannot silently weaken shared word or phrase policy."""
    _, rollout, _ = rollout_modules
    prohibited_phrase = "hand" + "-written"
    base = rollout.Dictionary(
        corrections=(("teh", "the"),),
        phrase_corrections=((prohibited_phrase, "handwritten"),),
    )

    merged = rollout.merge_dictionaries(base, rollout.Dictionary())

    assert merged.phrase_corrections == base.phrase_corrections, (
        "an empty local policy discarded the shared phrase correction"
    )

    with pytest.raises(ValueError, match="conflicting correction"):
        rollout.merge_dictionaries(
            base,
            rollout.Dictionary(corrections=(("teh", "ten"),)),
        )
    with pytest.raises(ValueError, match="conflicting phrase correction"):
        rollout.merge_dictionaries(
            base,
            rollout.Dictionary(phrase_corrections=((prohibited_phrase, "other"),)),
        )


def test_render_and_write_are_deterministic_valid_toml(
    rollout_modules: tuple[types.ModuleType, types.ModuleType, types.ModuleType],
    tmp_path: Path,
) -> None:
    """Rendering is stable, parseable and atomically installed."""
    _, rollout, _ = rollout_modules
    dictionary = rollout.Dictionary(
        stems=("organ",),
        accepted=("proper-name",),
        ignore_patterns=("https?://",),
        excluded_files=("target",),
    )
    output = tmp_path / "nested" / "typos.toml"

    first = rollout.render_typos_config(dictionary)
    rollout.write_config(output, dictionary)

    assert first == rollout.render_typos_config(dictionary), "rendering was unstable"
    assert output.read_text(encoding="utf-8") == first, "written config changed"
    assert tomllib.loads(first)["default"]["locale"] == "en-gb", (
        "rendered locale was not en-gb"
    )
    assert list(output.parent.glob(".typos.toml.*")) == [], (
        "atomic write left a temporary file"
    )
