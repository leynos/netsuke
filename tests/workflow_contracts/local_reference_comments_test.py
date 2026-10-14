"""Keep source-language comments out of local-reference matching."""

import pytest
from local_reference_comments import strip_reference_comments


@pytest.mark.parametrize(
    ("suffix", "source", "visible_text", "comment_paths"),
    [
        (
            ".rs",
            r"""let slash = '/';
let raw = r##"scripts/quoted.sh"##; // scripts/line.sh
let escaped_quote = '\'';
let unicode = '\u{1F600}';
/* outer /* scripts/nested.sh */ scripts/block.sh */
""",
            (
                "'/'",
                'r##"scripts/quoted.sh"##',
                r"'\''",
                r"'\u{1F600}'",
            ),
            ("scripts/line.sh", "scripts/nested.sh", "scripts/block.sh"),
        ),
        (
            ".toml",
            r"""homepage = "https://example.test/#literal" # scripts/inline.sh
path = 'scripts/quoted.sh # literal' # scripts/trailing.sh
# scripts/whole-line.sh
""",
            ("https://example.test/#literal", "scripts/quoted.sh # literal"),
            (
                "scripts/inline.sh",
                "scripts/trailing.sh",
                "scripts/whole-line.sh",
            ),
        ),
    ],
    ids=["rust-literals-and-nested-comments", "quoted-hash-values"],
)
def test_comment_paths_are_masked_without_hiding_string_paths(
    suffix: str,
    source: str,
    visible_text: tuple[str, ...],
    comment_paths: tuple[str, ...],
) -> None:
    """Preserve reference-like strings while masking language comments."""
    masked = strip_reference_comments(source, suffix)

    assert all(text in masked for text in visible_text), (
        "string paths must remain visible"
    )
    assert all(path not in masked for path in comment_paths), (
        "comment paths must be masked"
    )
    assert masked.count("\n") == source.count("\n"), (
        "masking must preserve line positions"
    )
