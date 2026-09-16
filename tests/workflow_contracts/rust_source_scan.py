"""Mask non-code Rust source without shifting executable-text positions."""

import re

RAW_STRING_START = re.compile(r'(?:br|r)(?P<hashes>#*)"')


def _masked(text: str) -> str:
    """Return spaces matching `text`, retaining its line boundaries."""
    return "".join("\n" if character == "\n" else " " for character in text)


def _line_comment_end(source: str, index: int) -> int | None:
    """Return the end of a line comment beginning at `index`."""
    if not source.startswith("//", index):
        return None
    end = source.find("\n", index)
    return len(source) if end < 0 else end


def _block_comment_end(source: str, index: int) -> int | None:
    """Return the end of a possibly nested block comment at `index`."""
    if not source.startswith("/*", index):
        return None
    depth, end = 1, index + 2
    while depth and end < len(source):
        if source.startswith("/*", end):
            depth, end = depth + 1, end + 2
        elif source.startswith("*/", end):
            depth, end = depth - 1, end + 2
        else:
            end += 1
    return end


def _quoted_literal_end(source: str, index: int, quote: str) -> int | None:
    """Return the end of a quoted literal beginning with `quote` at `index`."""
    if source[index] != quote:
        return None
    end = index + 1
    while end < len(source):
        if source[end] == "\\":
            end += 2
        elif source[end] == quote:
            return end + 1
        else:
            end += 1
    return len(source)


def _raw_string_end(source: str, index: int) -> int | None:
    """Return the end of a raw Rust string beginning at `index`."""
    match = RAW_STRING_START.match(source, index)
    if match is None:
        return None
    terminator = f'"{match.group("hashes")}'
    end = source.find(terminator, match.end())
    return len(source) if end < 0 else end + len(terminator)


def _non_code_end(source: str, index: int) -> int | None:
    """Return the end of non-code text beginning at `index`, when present."""
    for boundary in (
        _line_comment_end,
        _block_comment_end,
        _raw_string_end,
    ):
        if (end := boundary(source, index)) is not None:
            return end
    if (end := _quoted_literal_end(source, index, '"')) is not None:
        return end
    return _quoted_literal_end(source, index, "'")


def mask_non_code(source: str, retained_literals: set[str]) -> str:
    """Mask Rust comments and literals except tokens needed for command discovery."""
    masked = list(source)
    index = 0
    while index < len(source):
        end = _non_code_end(source, index)
        if end is None:
            index += 1
            continue
        literal = source[index:end]
        if literal not in retained_literals:
            masked[index:end] = _masked(literal)
        index = end
    return "".join(masked)
