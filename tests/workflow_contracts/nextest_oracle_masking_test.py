"""Hold the Nextest oracle's Rust masker to the text it must blank out.

`_nextest_oracle.listing.parameterized_tests` predicts how many instances a
parameterized test declares by reading the Rust sources directly, and compares
that against what Nextest reports. The prediction is what makes the comparison
evidence rather than a tautology, so it has to be right: a `#[case]` the masker
fails to blank out, or code it blanks out by mistake, silently moves the
expected count and the assertion stops meaning anything.

The masker is the part of that reading which is easy to get wrong, because
strings and comments nest inside one another textually. Clearing comments
before literals makes a literal holding `//` lose its closing quote and blank
the rest of its line, dropping the case it was written in; clearing literals
first makes a comment holding an unmatched quote run on into the code after it.
Neither order is correct, and both fail silently: the count moves *down*, which
reads as a test that declares fewer cases rather than as a broken reader.

The assertions below are therefore about what survives masking, expressed as
what the reader counts -- not about which substrings appear. A truncated
attribute still contains its own name, so an assertion that looks for one
passes against a masker that destroyed the line it sat on.

What is asserted here is only the masking, which is pure text handling and
needs no build. The counts themselves are compared against Nextest on the
coverage lane, by `.github/scripts/verify_nextest_anchored_filters.py`.

Run via ``make test-workflow-contracts``.
"""

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
# The oracle lives beside the coverage-lane script it serves, outside any
# package the test tree can import by name. `verify_nextest_anchored_filters.py`
# resolves it the same way for the same reason; the path is inserted rather
# than the module copied, so the test exercises the code that actually runs.
sys.path.insert(0, str(REPO_ROOT / ".github" / "scripts"))

from _nextest_oracle.listing import (  # ruff: ignore[module-import-not-at-top-of-file] - needs the sys.path insertion above.
    CASE_ATTRIBUTE,
    RUST_FUNCTION,
    masked,
)


def _cases(source: str) -> int:
    """Return how many `#[case]` attributes the reader counts in ``source``."""
    return len(CASE_ATTRIBUTE.findall(masked(source)))


def _declared(source: str) -> dict[str, int]:
    """Return the reader's own prediction for ``source``, as it computes it.

    The count is taken through the same two patterns `parameterized_tests`
    applies, so the assertion measures what the reader would report rather than
    a restatement of it. A test whose attributes survive only in truncated form
    is not found as a function at all -- which is why the attribute-level count
    below is not enough on its own.

    Returns
    -------
    dict[str, int]
        Each function name the reader finds to the number of `#[case]`
        attributes it counts there.
    """
    found: dict[str, int] = {}
    for match in RUST_FUNCTION.finditer(masked(source)):
        cases = CASE_ATTRIBUTE.findall(match.group("attributes"))
        if cases:
            found[match.group("name")] = len(cases)
    return found


def test_an_attribute_is_blanked_from_a_string_literal() -> None:
    """A `#[case]` written inside a fixture string is not a real case.

    The UI fixtures hold Rust sources as string literals, so a fixture holding
    an attribute line would otherwise be counted as a declared case and inflate
    the expected instance count.
    """
    source = '"src/lib.rs", "#[case::fixture(1)]\\nfn real() {}\\n", &[]\n'
    assert _cases(source) == 0, "an attribute inside a string literal is not a case"


def test_a_literal_holding_a_line_comment_keeps_its_case() -> None:
    """A `//` inside a literal must not consume the literal's closing quote.

    This is the ordering trap in the direction the code shipped with. Masking
    comments first makes the `//` run to the end of the line, taking the closing
    quote with it, so the attribute truncates and the reader finds no function
    there at all.

    The assertion is on the reader's own prediction rather than on the
    attribute count, because the truncated line still spells
    `#[case::commented_plain(` and still contains a `#[case]` token: counting
    attributes alone passes on the broken reader. Only the function-level parse
    moves, and that is the number the gate compares against Nextest.
    """
    source = (
        '    #[case::commented_plain("// mod wired;", false)]\n'
        "    fn literal_holding_a_comment(\n"
        "        #[case] path: &str,\n"
        "    ) -> Result<()> {\n"
        "        Ok(())\n"
        "    }\n"
    )
    assert _declared(source) == {"literal_holding_a_comment": 1}, (
        "a literal containing `//` must not cost the reader the function that "
        "declares it"
    )


def test_a_comment_holding_an_unmatched_quote_does_not_swallow_code() -> None:
    """A quote opened in a comment must not run on into a later literal.

    This is the trap in the other direction, and the one a naive swap
    introduces: clearing literals before comments lets a `"` opened in prose
    pair with the next `"` in the file, blanking everything between them --
    including any declaration that lies in the gap. Two quotes are needed to
    show it, because a lone one cannot pair with anything.
    """
    source = '// say "hello\nfn real() { let s = "x"; }\n#[case::live(1)]\n'
    assert "fn real()" in masked(source), (
        "a quote in a comment must not blank the declaration after it"
    )
    assert _cases(source) == 1, "the case after the comment must still be counted"


def test_a_comment_holding_a_matched_quote_pair_is_still_prose() -> None:
    """A pair of quotes in prose is prose, not a literal worth keeping."""
    source = '// e.g. "a" and "b"\n#[case::live(1)]\n'
    assert _cases(source) == 1, (
        "a bare pair of quotes in prose must not open a literal, so the case "
        "after the comment still counts"
    )


def test_a_raw_string_hides_its_contents_entirely() -> None:
    """Raw strings are everywhere in the fixtures and must be consumed whole.

    Reading `r#"…"#` as a plain literal would stop at the inner quote and
    resume mid-string, exposing fixture text as code -- which is how a raw
    string containing a case attribute becomes a phantom case that no run-time
    check can ever match.
    """
    source = 'let x = r#"#[case::ghost(1)] // "q""#;\n#[case::live(1)]\n'
    assert _cases(source) == 1, (
        "a raw string's contents must be masked, including any quotes inside it"
    )
    assert "ghost" not in masked(source), (
        "the raw string's body must be blanked whole, not merely its fence"
    )


def test_a_raw_string_with_a_longer_fence_is_not_closed_early() -> None:
    """The terminator's fence length is honoured, so `r##"…#"##` is one span."""
    source = 'let x = r##"a "# #[case::ghost(1)]"##;\n#[case::live(1)]\n'
    assert _cases(source) == 1, (
        "a raw string opened with two hashes must close on two, not one"
    )


def test_masking_preserves_length_and_line_positions() -> None:
    """Offsets survive masking, so a reported line still locates its source."""
    source = '// comment\nlet a = "text";\nfn real() {}\n'
    result = masked(source)
    assert len(result) == len(source), "masking must not change the length"
    assert result.count("\n") == source.count("\n"), (
        "masking must not change line boundaries"
    )


def test_the_masker_blanks_recognised_spans() -> None:
    """Liveness: every assertion above is an absence, which an identity passes.

    A `masked` that returned its input unchanged would satisfy "this text is
    gone" only if it also blanked nothing. This pairs the absence checks with a
    positive one, so a masker that stopped masking altogether cannot pass.
    """
    source = '// comment\nlet a = "text";\nfn real() {}\n'
    assert masked(source) != source, "the masker must blank what it recognises"
    assert "comment" not in masked(source), (
        "the line comment must be blanked, not passed through"
    )
    assert "text" not in masked(source), (
        "the string literal's contents must be blanked, not passed through"
    )
    assert "fn real() {}" in masked(source), "code must survive masking"
