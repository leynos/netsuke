"""Unit tests for the ``typos.toml`` generator script.

The generator is a standalone ``uv run`` script rather than a package
module, so it is loaded here through ``importlib`` from its file path.
"""

import importlib.util
import pathlib
import tempfile
import tomllib
import types
import typing as typ

import pytest
from hypothesis import given
from hypothesis import strategies as st

SCRIPT_PATH = pathlib.Path(__file__).resolve().parents[1] / "generate_typos_config.py"
REPOSITORY_ROOT = pathlib.Path(__file__).resolve().parents[2]
SAFE_FILENAME_CHARS = tuple("abcdefghijklmnopqrstuvwxyz0123456789_-")
SAFE_TYPOS_FILENAMES = st.lists(
    st.sampled_from(SAFE_FILENAME_CHARS),
    min_size=1,
    max_size=32,
).map(lambda chars: f"{''.join(chars)}.toml")


@pytest.fixture(name="generator", scope="module")
def generator_fixture() -> types.ModuleType:
    """Load the generator script as a module from its file path."""
    spec = importlib.util.spec_from_file_location("generate_typos_config", SCRIPT_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    assert isinstance(module, types.ModuleType)
    return module


@pytest.fixture(name="rendered_config", scope="module")
def rendered_config_fixture(generator: types.ModuleType) -> str:
    """Render the generator output once for property tests."""
    return typ.cast("str", generator.render_config())


def test_render_config_emits_every_stem_and_suffix_pair(
    generator: types.ModuleType,
) -> None:
    """Every stem inflection gets an -ise correction and an -ize identity."""
    rendered = generator.render_config()
    for stem in generator.STEMS:
        for ise, ize in generator.SUFFIX_PAIRS:
            assert f'{stem}{ise} = "{stem}{ize}"' in rendered
            assert f'{stem}{ize} = "{stem}{ize}"' in rendered


def test_render_config_accepts_extra_words_verbatim(
    generator: types.ModuleType,
) -> None:
    """Every extra accepted word gets an identity entry."""
    rendered = generator.render_config()
    for word in generator.EXTRA_ACCEPTED_WORDS:
        assert f'{word} = "{word}"' in rendered


def test_render_config_ends_with_trailing_newline(
    generator: types.ModuleType,
) -> None:
    """The rendered document ends with exactly one trailing newline."""
    rendered = generator.render_config()
    assert rendered.endswith("\n")
    assert not rendered.endswith("\n\n")


def test_render_config_parses_as_valid_toml(generator: types.ModuleType) -> None:
    """The rendered configuration parses as TOML with no duplicate keys.

    ``tomllib`` raises ``TOMLDecodeError`` on duplicate keys, so parsing guards
    against two stem/suffix combinations (or an extra accepted word) colliding
    into the same ``extend-words`` entry. The exact entry count additionally
    documents that every stem inflection and accepted word is present.
    """
    parsed = tomllib.loads(generator.render_config())
    extend_words = parsed["default"]["extend-words"]
    expected = len(generator.EXTRA_ACCEPTED_WORDS) + 2 * len(generator.STEMS) * len(
        generator.SUFFIX_PAIRS
    )
    assert len(extend_words) == expected


@given(data=st.data())
def test_render_config_property_emits_sampled_stem_suffix_pair(
    generator: types.ModuleType,
    rendered_config: str,
    data: st.DataObject,
) -> None:
    """A sampled stem and suffix pair gets correction and identity entries."""
    stem = data.draw(st.sampled_from(generator.STEMS))
    ise, ize = data.draw(st.sampled_from(generator.SUFFIX_PAIRS))

    assert f'{stem}{ise} = "{stem}{ize}"' in rendered_config
    assert f'{stem}{ize} = "{stem}{ize}"' in rendered_config


@given(data=st.data())
def test_render_config_property_accepts_sampled_extra_word(
    generator: types.ModuleType,
    rendered_config: str,
    data: st.DataObject,
) -> None:
    """A sampled extra accepted word gets an identity entry."""
    word = data.draw(st.sampled_from(sorted(generator.EXTRA_ACCEPTED_WORDS)))

    assert f'{word} = "{word}"' in rendered_config


@given(filename=SAFE_TYPOS_FILENAMES)
def test_main_property_writes_rendered_config_without_mutation(
    generator: types.ModuleType,
    filename: str,
) -> None:
    """main() writes rendered content exactly to a sampled safe filename."""
    with tempfile.TemporaryDirectory() as temporary_directory:
        output = pathlib.Path(temporary_directory) / filename
        generator.main(output)

        assert output.read_text(encoding="utf-8") == generator.render_config()


def test_main_writes_rendered_config_to_explicit_path(
    generator: types.ModuleType,
    tmp_path: pathlib.Path,
) -> None:
    """main() writes the rendered configuration to the given output path."""
    output = tmp_path / "typos.toml"
    generator.main(output)
    assert output.read_text(encoding="utf-8") == generator.render_config()


def test_main_default_path_resolves_to_repository_root(
    generator: types.ModuleType,
    tmp_path: pathlib.Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """main() defaults to typos.toml two levels above the script file."""
    fake_script = tmp_path / "scripts" / "generate_typos_config.py"
    monkeypatch.setattr(generator, "__file__", str(fake_script))
    generator.main()
    written = tmp_path / "typos.toml"
    assert written.read_text(encoding="utf-8") == generator.render_config()


def test_committed_config_matches_generator_output(
    generator: types.ModuleType,
) -> None:
    """The committed typos.toml must not drift from the generator."""
    committed = (REPOSITORY_ROOT / "typos.toml").read_text(encoding="utf-8")
    assert committed == generator.render_config()


def test_committed_config_parses_as_valid_toml() -> None:
    """The committed typos.toml parses as TOML (no syntax or duplicate-key errors)."""
    committed = (REPOSITORY_ROOT / "typos.toml").read_text(encoding="utf-8")
    parsed = tomllib.loads(committed)
    assert parsed["default"]["locale"] == "en-gb"
    assert parsed["default"]["extend-words"]
