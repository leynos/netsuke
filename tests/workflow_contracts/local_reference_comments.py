"""Mask language comments without hiding local references in source strings.

Root test files and reached scripts both contribute references to the local
item contract. The maskers are deliberately limited to the source languages
the contract scans, so comment-looking text inside strings remains visible.
"""

import io
import tokenize
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def strip_reference_comments(text: str, suffix: str | None) -> str:
    r"""Mask source-language comments while preserving text in strings.

    Returns
    -------
    str
        Source text with comments replaced by spaces.

    Examples
    --------
    >>> strip_reference_comments("// scripts/orphan.sh\n", ".rs").strip()
    ''

    """
    masker = _COMMENT_MASKERS.get(suffix or "")
    return text if masker is None else masker(text)


def _strip_python_comments(text: str) -> str:
    """Mask Python comment tokens without changing strings or line numbers."""
    lines = text.splitlines(keepends=True)
    try:
        tokens = tokenize.generate_tokens(io.StringIO(text).readline)
        for token_info in tokens:
            if token_info.type != tokenize.COMMENT:
                continue
            (row, start), (_, end) = token_info.start, token_info.end
            line = lines[row - 1]
            lines[row - 1] = line[:start] + " " * (end - start) + line[end:]
    except IndentationError, tokenize.TokenError:
        pass
    return "".join(lines)


def _strip_shell_comments(text: str) -> str:
    """Mask Bash comments outside single- and double-quoted strings."""
    characters = list(text)
    quote: str | None = None
    escaped = False
    index = 0
    while index < len(text):
        current = text[index]
        if escaped:
            escaped = False
        elif quote is not None:
            quote, escaped = _advance_shell_quote(quote, current)
        elif current in {"'", '"'}:
            quote = current
        elif current == "\\":
            escaped = True
        elif current == "#" and _starts_shell_comment(text, index):
            comment_end = _line_end(text, index)
            _mask_comment(characters, index, comment_end)
            index = comment_end - 1
        index += 1
    return "".join(characters)


def _strip_powershell_comments(text: str) -> str:
    """Mask PowerShell line and block comments outside quoted strings."""
    characters = list(text)
    quote: str | None = None
    escaped = False
    index = 0
    while index < len(text):
        current = text[index]
        if escaped:
            escaped = False
        elif quote is not None:
            quote, escaped, index = _advance_powershell_quote(
                text, index, quote, current
            )
        elif text.startswith("<#", index):
            comment_end = _block_comment_end(text, index)
            _mask_comment(characters, index, comment_end)
            index = comment_end - 1
        elif current in {"'", '"'}:
            quote = current
        elif current == "`":
            escaped = True
        elif current == "#":
            comment_end = _line_end(text, index)
            _mask_comment(characters, index, comment_end)
            index = comment_end - 1
        index += 1
    return "".join(characters)


def _strip_rust_comments(text: str) -> str:
    """Mask Rust comments while retaining normal and raw string literals."""
    characters = list(text)
    index = 0
    while index < len(text):
        raw_end = _raw_string_end(text, index)
        if raw_end is not None:
            index = raw_end
        elif text.startswith("//", index):
            comment_end = _line_end(text, index)
            _mask_comment(characters, index, comment_end)
            index = comment_end
        elif text.startswith("/*", index):
            comment_end = _rust_block_comment_end(text, index)
            _mask_comment(characters, index, comment_end)
            index = comment_end
        elif text[index] == '"':
            index = _quoted_string_end(text, index)
        elif text[index] == "'":
            char_end = _char_literal_end(text, index)
            index = index + 1 if char_end is None else char_end
        else:
            index += 1
    return "".join(characters)


def _strip_markdown_comments(text: str) -> str:
    """Mask HTML comments in Markdown while preserving line positions."""
    characters = list(text)
    index = 0
    while (start := text.find("<!--", index)) >= 0:
        end_marker = text.find("-->", start + 4)
        end = len(text) if end_marker < 0 else end_marker + 3
        _mask_comment(characters, start, end)
        index = end
    return "".join(characters)


def _strip_quoted_hash_comments(text: str) -> str:
    """Mask TOML and feature comments outside single- or double-quoted text."""
    characters = list(text)
    quote: str | None = None
    escaped = False
    for index, current in enumerate(text):
        if current == "\n":
            quote = None
            escaped = False
        elif escaped:
            escaped = False
        elif quote == '"' and current == "\\":
            escaped = True
        elif quote is not None and current == quote:
            quote = None
        elif quote is None and current in {"'", '"'}:
            quote = current
        elif _is_hash_comment_outside_quotes(text, index, quote, current):
            comment_end = _line_end(text, index)
            _mask_comment(characters, index, comment_end)
    return "".join(characters)


def _raw_string_end(text: str, index: int) -> int | None:
    """Return the end of a Rust raw string beginning at an ``r`` prefix."""
    prefix = index
    if text.startswith("br", index):
        prefix += 1
    if prefix >= len(text) or text[prefix] != "r":
        return None
    quote = text.find('"', prefix + 1)
    if quote < 0 or "\n" in text[prefix:quote]:
        return None
    hashes = text[prefix + 1 : quote]
    if hashes.strip("#"):
        return None
    terminator = '"' + hashes
    end = text.find(terminator, quote + 1)
    return len(text) if end < 0 else end + len(terminator)


def _rust_block_comment_end(text: str, start: int) -> int:
    """Find the end of a possibly nested Rust block comment."""
    depth = 1
    index = start + 2
    while index < len(text) and depth:
        if text.startswith("/*", index):
            depth += 1
            index += 2
        elif text.startswith("*/", index):
            depth -= 1
            index += 2
        else:
            index += 1
    return index


def _quoted_string_end(text: str, start: int) -> int:
    """Find the end of a Rust double-quoted string, including escapes."""
    escaped = False
    for index in range(start + 1, len(text)):
        current = text[index]
        if escaped:
            escaped = False
        elif current == "\\":
            escaped = True
        elif current == '"':
            return index + 1
    return len(text)


def _char_literal_end(text: str, start: int) -> int | None:
    """Recognise Rust character literals without mistaking lifetimes for them."""
    first_character = start + 1
    if first_character >= len(text) or text[first_character].isspace():
        return None
    if text[first_character] != "\\":
        end = first_character + 1
        return end + 1 if text[end : end + 1] == "'" else None
    for index in range(first_character + 1, len(text)):
        if text[index] == "\n" or text[index].isspace():
            return None
        if text[index] == "'":
            return index + 1
    return None


def _is_hash_comment_outside_quotes(
    text: str,
    index: int,
    quote: str | None,
    current: str,
) -> bool:
    """Identify a TOML, YAML or feature comment outside quoted content."""
    if quote is not None or current != "#":
        return False
    return index == 0 or text[index - 1].isspace()


def _advance_shell_quote(quote: str, current: str) -> tuple[str | None, bool]:
    """Advance over one Bash character inside a quoted string."""
    if quote == '"' and current == "\\":
        return quote, True
    if current == quote:
        return None, False
    return quote, False


def _advance_powershell_quote(
    text: str,
    index: int,
    quote: str,
    current: str,
) -> tuple[str | None, bool, int]:
    """Advance over one quoted PowerShell character and its escapes."""
    if quote == "'" and text[index : index + 2] == "''":
        return quote, False, index + 1
    if current == "`":
        return quote, True, index
    if current == quote:
        return None, False, index
    return quote, False, index


def _starts_shell_comment(text: str, index: int) -> bool:
    """Recognise Bash's comment marker at the start of an unquoted word."""
    if index == 0:
        return True
    previous = text[index - 1]
    return previous.isspace() or previous in ";|&()<>"


def _line_end(text: str, index: int) -> int:
    """Find the next line ending or the end of a source string."""
    end = text.find("\n", index)
    return len(text) if end < 0 else end


def _block_comment_end(text: str, index: int) -> int:
    """Find the end of a PowerShell block comment or the end of its source."""
    end = text.find("#>", index + 2)
    return len(text) if end < 0 else end + 2


def _mask_comment(characters: list[str], start: int, end: int) -> None:
    """Replace non-newline comment characters with spaces in place."""
    for index in range(start, end):
        if characters[index] != "\n":
            characters[index] = " "


_COMMENT_MASKERS: dict[str, cabc.Callable[[str], str]] = {
    ".py": _strip_python_comments,
    ".bash": _strip_shell_comments,
    ".sh": _strip_shell_comments,
    ".ps1": _strip_powershell_comments,
    ".rs": _strip_rust_comments,
    ".md": _strip_markdown_comments,
    ".toml": _strip_quoted_hash_comments,
    ".feature": _strip_quoted_hash_comments,
    ".yaml": _strip_quoted_hash_comments,
    ".yml": _strip_quoted_hash_comments,
}
