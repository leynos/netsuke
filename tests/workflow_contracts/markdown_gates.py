"""Reading the JSONC configuration the Markdown lint gate is written against.

Split from ``markdown_gates_test`` so both modules stay inside the 400-line
limit the lint gate enforces. ``.markdownlint-cli2.jsonc`` is JSONC, and this
repository's reader must be exactly as permissive as the linter or the gate
fails over a file the linter is happy with. markdownlint-cli2 parses it with
`jsonc-parser` in its default mode: `//` runs to end of line, `/* ... */`
spans lines, both may appear wherever whitespace may, and a trailing comma
before `}` or `]` is allowed. Only the last of those is not a comment, and
`json` accepts none of the three.

The reader is a scanner rather than a set of substitutions, because a
substitution cannot tell syntax from content. `**/*.md` and a URL both carry
`//` and `/*`, and every value in the configuration this reads is a glob, so a
reader that cut at a comment opener would corrupt the very values it is
reading for.
"""

import json

#: The comment openers JSONC allows and `json` does not. Both are stripped
#: wherever they appear, not only at the start of a line: markdownlint-cli2
#: parses this file with `jsonc-parser` in its default mode, which honors
#: `//` to end of line and `/* ... */` anywhere, and a reader that honored
#: less than the linter would reject a configuration the linter accepts.
COMMENT_OPENERS = ("//", "/*")


def _skip_comment(text: str, start: int) -> int:
    """Return the index just past the comment opening at ``start``."""
    if text.startswith("//", start):
        newline = text.find("\n", start)
        return len(text) if newline < 0 else newline
    terminator = text.find("*/", start + 2)
    return len(text) if terminator < 0 else terminator + 2


def _string_end(text: str, start: int) -> int:
    """Return the index just past the string literal opening at ``start``.

    Scanned rather than searched for the next quote, because a backslash
    escapes the character after it and an escaped quote does not close the
    literal.

    Returns
    -------
    int
        The index just past the closing quote, or ``len(text)`` when the
        literal is unterminated.
    """
    index = start + 1
    while index < len(text):
        match text[index]:
            case "\\":
                index += 2
            case '"':
                return index + 1
            case _:
                index += 1
    return len(text)


def _next_significant(text: str, start: int) -> str:
    """Return the first character after ``start`` that is not a comment or space."""
    index = start
    while index < len(text):
        if text.startswith(COMMENT_OPENERS, index):
            index = _skip_comment(text, index)
        elif text[index].isspace():
            index += 1
        else:
            return text[index]
    return ""


def without_comments(text: str) -> str:
    """Return JSONC text as JSON: comments dropped, trailing commas removed.

    A string is copied through whole, so `//` or `/*` inside a glob or a URL
    survives, and both comment forms are dropped wherever they appear rather
    than only on a line of their own.

    Returns
    -------
    str
        ``text`` with every comment removed and every trailing comma before a
        closing brace or bracket dropped, ready for ``json.loads``.
    """
    out: list[str] = []
    index = 0
    while index < len(text):
        character = text[index]
        if character == '"':
            end = _string_end(text, index)
            out.append(text[index:end])
            index = end
        elif text.startswith(COMMENT_OPENERS, index):
            index = _skip_comment(text, index)
        elif character == "," and _next_significant(text, index + 1) in {"}", "]"}:
            index += 1
        else:
            out.append(character)
            index += 1
    return "".join(out)


def read_jsonc(text: str) -> object:
    """Return the object ``text`` declares, comments and trailing commas aside."""
    return json.loads(without_comments(text))
