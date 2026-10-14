"""Check that the generated spelling ignore recognizes only color options."""

import re
import tomllib
from pathlib import Path

import pytest
from hypothesis import given, settings
from hypothesis import strategies as st

ROOT = Path(__file__).resolve().parents[2]
PREFIX_CHARACTERS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-"
SUPPORTED_SUFFIXES = ("", "ed", "s", "ing", "ize", "ized", "izes", "izing")


def _generated_color_option_pattern() -> re.Pattern[str]:
    """Load the color-option expression from the generated Typos config."""
    config = tomllib.loads((ROOT / "typos.toml").read_text(encoding="utf-8"))
    expressions = config["default"]["extend-ignore-re"]
    matches = [
        expression
        for expression in expressions
        if expression.startswith("--") and "color(?:" in expression
    ]
    assert len(matches) == 1, "expected one generated color-option ignore pattern"
    return re.compile(matches[0])


COLOR_OPTION_PATTERN = _generated_color_option_pattern()


@pytest.mark.parametrize(
    "option",
    [
        "--color",
        "--colored",
        "--colors",
        "--coloring",
        "--colorize",
        "--colorized",
        "--colorizes",
        "--colorizing",
        "--no-color",
        "--coloring-mode",
        "--color=always",
        "--color_option",
    ],
)
def test_generated_pattern_matches_supported_color_options(option: str) -> None:
    """Match the color option spellings covered by the generated ignore."""
    assert COLOR_OPTION_PATTERN.search(option), f"expected {option!r} to match"


@pytest.mark.parametrize(
    "option",
    [
        "--colour",
        "--colorful",
        "--colorblind",
        "--coloringless",
        "--colorizeable",
        "--colorx",
        "--color0",
    ],
)
def test_generated_pattern_rejects_near_miss_options(option: str) -> None:
    """Reject color spellings outside the generated ignore's language."""
    assert COLOR_OPTION_PATTERN.search(option) is None, (
        f"unexpected match for {option!r}"
    )


@settings(max_examples=100)
@given(
    prefix=st.text(alphabet=PREFIX_CHARACTERS, max_size=16),
    suffix=st.sampled_from(SUPPORTED_SUFFIXES),
    boundary=st.sampled_from(("", "_option", "=always", " ")),
)
def test_generated_pattern_property_matches_supported_shapes(
    prefix: str, suffix: str, boundary: str
) -> None:
    """Match generated combinations of valid prefixes and suffixes."""
    option = f"--{prefix}color{suffix}{boundary}"
    assert COLOR_OPTION_PATTERN.search(option), f"expected {option!r} to match"


@settings(max_examples=100)
@given(
    prefix=st.text(alphabet=PREFIX_CHARACTERS, max_size=16),
    invalid_character=st.sampled_from((".", "/", "!", "+")),
)
def test_generated_pattern_property_rejects_invalid_prefix_characters(
    prefix: str, invalid_character: str
) -> None:
    """Reject prefixes containing characters outside the configured alphabet."""
    option = f"--{prefix}{invalid_character}color"
    assert COLOR_OPTION_PATTERN.search(option) is None, (
        f"unexpected match for {option!r}"
    )


@settings(max_examples=100)
@given(
    prefix=st.text(alphabet=PREFIX_CHARACTERS, max_size=16),
    suffix=st.sampled_from(("ful", "blind", "less", "ward", "x", "0")),
    continuation=st.sampled_from(
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
    ),
)
def test_generated_pattern_property_rejects_unsupported_continuations(
    prefix: str, suffix: str, continuation: str
) -> None:
    """Reject unsupported suffixes followed by word characters."""
    option = f"--{prefix}color{suffix}{continuation}"
    assert COLOR_OPTION_PATTERN.search(option) is None, (
        f"unexpected match for {option!r}"
    )
