"""Generate Rust sources and assert what the oracle's masker does to them.

`_nextest_oracle.listing.parameterized_tests` predicts how many instances a
parameterized test declares by masking the Rust sources and counting their
`#[case]` attributes. The prediction is the independent reading that makes the
comparison against Nextest evidence rather than a tautology, so the masker
carries the whole check: a fragment it blanks by mistake drops a real case, and
a fragment it fails to blank invents a phantom one. Either way the expected
count moves and the assertion downstream stops meaning anything, silently.

`nextest_oracle_masking_test` pins the known lexer traps as fixed examples --
the `//` inside a literal, the unmatched quote in prose, the raw-string fence.
Those are the cases someone thought of. This module states the invariants that
must hold for *any* arrangement of fragments, and lets Hypothesis search the
arrangements nobody wrote down.

The two alphabets are disjoint on purpose, and that is load-bearing rather
than decorative. "The body is gone" is only checkable as a substring
assertion while no body text can also occur in the surviving code; with a
shared alphabet, a one-character body such as `x` reappears inside an
unrelated `fn x()`, and the assertion reports a correct masker as broken. So
fragment bodies are drawn from `xyz` and code from `abc`.

Each generated source is a sequence of whole fragments rather than arbitrary
text. That keeps every sample inside the language the masker reads: arbitrary
characters would mostly generate unlexable input, and the properties would
then be asserting things about nonsense rather than about the lexer.

Run via ``make test-workflow-contracts``.
"""

import sys
from pathlib import Path

from hypothesis import given, settings
from hypothesis import strategies as st

REPO_ROOT = Path(__file__).resolve().parents[2]
# The oracle lives beside the coverage-lane script it serves, outside any
# package the test tree can import by name. `verify_nextest_anchored_filters.py`
# resolves it the same way for the same reason; the path is inserted rather
# than the module copied, so the test exercises the code that actually runs.
sys.path.insert(0, str(REPO_ROOT / ".github" / "scripts"))

from _nextest_oracle.listing import masked  # ruff: ignore[module-import-not-at-top-of-file] - needs the sys.path insertion above.

#: Fragment bodies, drawn only from characters that cannot open or terminate a
#: fragment and cannot occur in generated code. See the module docstring.
BODIES = st.text(alphabet="xyz", min_size=1, max_size=4)

#: Code text, disjoint from every body alphabet for the same reason.
CODE_TEXT = st.text(alphabet="abc", min_size=1, max_size=4)

#: The non-code constructs the masker recognises, each rendered around a body.
#: `nested_block` reaches the depth counter that a non-nesting reader gets
#: wrong; the two raw-string spellings reach the fence-length rule.
FRAGMENT_SPELLINGS = {
    "line": "//{body}",
    "block": "/*{body}*/",
    "nested_block": "/*{body}/*{body}*/{body}*/",
    "string": '"{body}"',
    "raw": 'r"{body}"',
    "raw_hash": 'r##"{body}"##',
    "char": "'{body_mark}'",
}


@st.composite
def fragments(draw: st.DrawFn) -> tuple[str, list[tuple[bool, str]]]:
    """Draw a source of whole fragments, with a manifest of what each one is.

    Returns
    -------
    tuple[str, list[tuple[bool, str]]]
        The joined source, and one ``(is_code, text)`` pair per fragment in
        the order it appears, so a property can assert per fragment rather
        than only over the whole text.
    """
    parts = draw(
        st.lists(
            st.one_of(
                st.tuples(st.sampled_from(sorted(FRAGMENT_SPELLINGS)), BODIES).map(
                    lambda kb: (False, _spell(kb[0], kb[1]))
                ),
                st.tuples(CODE_TEXT, CODE_TEXT).map(
                    lambda ab: (True, f"fn {ab[0]}() {{ let _ = {ab[1]}; }}")
                ),
            ),
            min_size=1,
            max_size=4,
        )
    )
    return "\n".join(text for _, text in parts), parts


def _spell(kind: str, body: str) -> str:
    """Return ``kind``'s fragment spelled around ``body``."""
    return FRAGMENT_SPELLINGS[kind].format(body=body, body_mark=body[0])


def _body_of(text: str) -> str:
    """Return the payload of a non-code fragment, with its delimiters removed."""
    return text.strip("\"'#/*")


@settings(max_examples=400, derandomize=True, deadline=None)
@given(fragments())
def test_masking_preserves_offsets_for_any_fragment_sequence(
    source: tuple[str, list[tuple[bool, str]]],
) -> None:
    """Keep length and every newline, whatever the fragments are.

    Offsets are what make a reported line locatable, and the masker blanks a
    span by replacing it with spaces of equal length. A reader that dropped or
    added a character would move every later column, and one that swallowed a
    newline would move every later line.
    """
    text, _ = source
    result = masked(text)
    assert len(result) == len(text), "masking must not change the length"
    assert result.count("\n") == text.count("\n"), (
        "masking must not change line boundaries"
    )


@settings(max_examples=400, derandomize=True, deadline=None)
@given(fragments())
def test_code_survives_and_non_code_bodies_do_not(
    source: tuple[str, list[tuple[bool, str]]],
) -> None:
    """Blank every fragment body, keep every code fragment whole.

    This is the pair that makes the masker's effect observable in both
    directions. Asserting only that bodies are gone would pass a masker that
    blanked the entire file; asserting only that code survives would pass one
    that blanked nothing.
    """
    text, parts = source
    result = masked(text)
    for is_code, fragment in parts:
        if is_code:
            assert fragment in result, (
                f"code must survive masking: {fragment!r} was dropped from "
                f"{text!r}, leaving {result!r}"
            )
        else:
            body = _body_of(fragment)
            assert body not in result, (
                f"a fragment body must be blanked: {body!r} from {fragment!r} "
                f"survived in {result!r}"
            )


@settings(max_examples=400, derandomize=True, deadline=None)
@given(fragments())
def test_masking_is_idempotent(source: tuple[str, list[tuple[bool, str]]]) -> None:
    """Masking an already-masked source changes nothing further.

    The masker replaces a span with spaces, so re-reading its own output must
    find no fragment left to consume. A reader that failed this would be
    discovering constructs in text that contains none -- which is what a
    mis-detected delimiter does, and it is the difference between a blanked
    span and a corrupted one.
    """
    text, _ = source
    once = masked(text)
    assert masked(once) == once, (
        f"masking must be idempotent: {once!r} masked again gives {masked(once)!r}"
    )
