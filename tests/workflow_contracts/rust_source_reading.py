"""Read Rust source as code, with its comments and literals set aside.

A contract that searches raw source finds what the source talks about as
readily as what it does. `// trybuild::TestCases::new()` in a paragraph
explaining a removed harness reads exactly like the harness, and a repository
that documents why something is absent is the repository most likely to carry
that sentence.

One scan answers the question, because the four contexts are not independent:
a `//` inside a string is not a comment, a quote inside a comment does not open
a string, and a `"` inside a raw string closes nothing until the matching hash
count arrives. Reading them separately means reading each with the others'
rules switched off, which is how a quote inside a raw string exposes a `//`
that blanks the rest of a line.

The scan preserves length, replacing every non-code character with a space, so
a line number or an offset taken from the result still names the source.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

__all__ = ["code_only"]

#: The longest a character literal may be, counting the closing quote:
#: `'\u{1F600}'` is the widest form Rust accepts.
_MAX_CHAR_LITERAL: typ.Final[int] = 12


def _blank(text: str) -> str:
    """Return `text` with every character but a newline replaced by a space."""
    return "".join("\n" if character == "\n" else " " for character in text)


def _skip_line_comment(text: str, start: int) -> int:
    """Return the index just past a `//` comment starting at `start`."""
    end = text.find("\n", start)
    return len(text) if end == -1 else end


#: What each block-comment delimiter does to the nesting depth. Rust nests
#: block comments, so a `/*` inside one opens a comment the outer `*/` does not
#: close.
_BLOCK_DEPTH: typ.Final[dict[str, int]] = {"/*": 1, "*/": -1}


def _skip_block_comment(text: str, start: int) -> int:
    """Return the index just past a `/* */` comment, which may nest.

    Parameters
    ----------
    text
        The whole source.
    start
        The index of the opening `/*`.

    Returns
    -------
    int
        The index just past the matching `*/`, or the end of the text when
        nothing closes the comment.
    """
    # The opening delimiter is consumed before the loop, so the depth is never
    # zero inside it and the loop's own condition is the whole answer. Only
    # `_region_end` calls this, and only when `/*` sits at `start`.
    depth = 1
    index = start + 2
    while depth:
        if index >= len(text):
            return len(text)
        step = _BLOCK_DEPTH.get(text[index : index + 2], 0)
        depth += step
        index += 2 if step else 1
    return index


def _skip_quoted(text: str, start: int) -> int:
    r"""Return the index just past a `"`-delimited string with `\` escapes."""
    index = start + 1
    while index < len(text):
        if text[index] == "\\":
            index += 2
            continue
        if text[index] == '"':
            return index + 1
        index += 1
    return len(text)


def _skip_raw(text: str, start: int, hashes: int) -> int:
    """Return the index just past a raw string closed by `"` and `hashes` #."""
    closing = '"' + "#" * hashes
    end = text.find(closing, start)
    return len(text) if end == -1 else end + len(closing)


#: The prefixes a raw string may carry before its `r`. Rust 2024 adds the raw
#: C string `cr#"..."#`, whose inner quote closes nothing without the matching
#: hashes; a scanner that refused the `c` read the opening quote as an ordinary
#: string delimiter and exposed the rest of the literal as code.
_RAW_PREFIXES: typ.Final[tuple[str, ...]] = ("b", "c")


def _raw_opening(text: str, index: int) -> tuple[int, int] | None:
    """Return the body start and hash count of a raw string opening here."""
    # None is the common case: `r`, `b` and `c` are ordinary identifier
    # characters, and the caller's `_identifier_before` guard is what keeps a
    # prefix inside a longer name from opening a literal.
    cursor = index
    if text.startswith(_RAW_PREFIXES, cursor):
        cursor += 1
    if not text.startswith("r", cursor):
        return None
    cursor += 1
    hashes = 0
    while text.startswith("#", cursor):
        hashes += 1
        cursor += 1
    if not text.startswith('"', cursor):
        return None
    return cursor + 1, hashes


def _is_char_literal(text: str, index: int) -> int | None:
    """Return the index past a character literal, or `None` for a lifetime."""
    # A lone `'` is how Rust writes a lifetime, and `'static` is not an
    # unclosed literal. The two are told apart by looking for a closing quote
    # within the widest literal Rust accepts; beyond that the quote is a
    # lifetime and the text after it is the code it looks like.
    index += 1
    if text.startswith("\\", index):
        index += 2
    elif index < len(text):
        index += 1
    limit = min(len(text), index + _MAX_CHAR_LITERAL)
    while index < limit:
        if text[index] == "'":
            return index + 1
        index += 1
    return None


def _identifier_before(text: str, index: int) -> bool:
    """Return whether the character before `index` may end an identifier."""
    return index > 0 and (text[index - 1].isalnum() or text[index - 1] == "_")


def _region_end(text: str, index: int) -> int | None:
    """Return the index just past a non-code region opening at `index`.

    The one place the four contexts are told apart, so `code_only` is a walk
    and this is the grammar. Order matters: a raw string is tried before a bare
    quote, or `r#"` reads as an identifier followed by a string.

    Parameters
    ----------
    text
        The whole source.
    index
        Where to look.

    Returns
    -------
    int or None
        The index just past the region, or None when code begins here. A lone
        `'` that closes nothing is a lifetime and answers None, so the text
        after it is read as the code it is.
    """
    if text.startswith("//", index):
        return _skip_line_comment(text, index)
    if text.startswith("/*", index):
        return _skip_block_comment(text, index)
    opening = None if _identifier_before(text, index) else _raw_opening(text, index)
    if opening is not None:
        body, hashes = opening
        return _skip_raw(text, body, hashes)
    if text[index] == '"':
        return _skip_quoted(text, index)
    if text[index] == "'":
        return _is_char_literal(text, index)
    return None


def code_only(text: str) -> str:
    """Return `text` with comments and literals replaced by whitespace.

    Length and line structure are preserved, so an offset into the result names
    the same place in the source.

    Parameters
    ----------
    text
        Rust source, as read from a file.

    Returns
    -------
    str
        The same text with every comment, string, raw string, byte string and
        character literal blanked.

    Examples
    --------
    >>> code_only('let x = 1; // TestCases::new()').rstrip()
    'let x = 1;'
    >>> 'TestCases' in code_only('let s = "TestCases::new()"; let y = 2;')
    False
    >>> code_only('let s = "ab"; let y = 2;')
    'let s =     ; let y = 2;'
    """
    out: list[str] = []
    index = 0
    while index < len(text):
        end = _region_end(text, index)
        if end is None:
            out.append(text[index])
            index += 1
            continue
        out.append(_blank(text[index:end]))
        index = end
    return "".join(out)
