"""Read the wrapper examples out of the documents that publish them.

Three documents show a developer how to run Kani under a `systemd-run` scope:
`docs/developers-guide.md` and the 4.2.2 and 4.2.3 ExecPlans. The contracts in
``kani_scope_wrapper_docs_test.py`` assert against those examples, and this
module is how they are found and split into their parts.

Reading rather than regex-matching the whole block is what makes the checks
mean anything. A wrapper is a command whose *arguments* carry the bounds and
whose *payload* carries the pipeline, and a search over raw text cannot tell
those apart: a comment quoting `MemoryMax=8G` would satisfy a bound the
command does not set, and a `pipefail` written after the closing quote would
satisfy a contract about the scope's payload.

Split from ``kani_scope_wrapper_docs_test.py`` to keep both modules inside the
repository's 400-line limit.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

if typ.TYPE_CHECKING:
    from pathlib import Path

#: An opening fence: up to three spaces of indent, three or more of one
#: character, then an optional info string.
#:
#: The fence may be indented because the 4.2.2 plan carries its template inside
#: a list item, so requiring column zero would silently skip it. Both fence
#: characters are accepted because CommonMark allows either, and a block the
#: scanner cannot see is a block no contract checks.
OPENING_FENCE = re.compile(r"^ {0,3}(?P<fence>`{3,}|~{3,})(?P<info>.*)$")


def fenced_blocks(path: Path) -> list[str]:
    """Return the body of every fenced code block in ``path``.

    Implements the CommonMark rules that decide where a block ends: a closing
    fence uses the same character as its opener, is at least as long, and is
    followed only by whitespace. That is stricter than "the next line starting
    with three backticks", which would end a block early on a shorter fence and
    pair the wrong opener with the wrong closer.

    Returns
    -------
    list[str]
        The text between each pair of fences, in document order.
    """
    blocks: list[str] = []
    lines = path.read_text(encoding="utf-8").splitlines()
    index = 0
    while index < len(lines):
        opening = OPENING_FENCE.match(lines[index])
        if opening is None:
            index += 1
            continue
        fence = opening.group("fence")
        body: list[str] = []
        index += 1
        while index < len(lines):
            if closes_block(lines[index], fence):
                break
            body.append(lines[index])
            index += 1
        blocks.append("\n".join(body))
        index += 1
    return blocks


def closes_block(line: str, fence: str) -> bool:
    """Whether ``line`` closes a block opened by ``fence``.

    Returns
    -------
    bool
        True when the line is the same character, at least as long, and has
        nothing but whitespace after it.
    """
    stripped = line.strip()
    if not stripped or stripped[0] != fence[0]:
        return False
    run = len(stripped) - len(stripped.lstrip(fence[0]))
    return run >= len(fence) and not stripped[run:].strip()


def wrapper_blocks(path: Path) -> list[str]:
    """Return the fenced blocks of ``path`` that invoke ``systemd-run``.

    Returns
    -------
    list[str]
        The blocks mentioning `systemd-run`, in document order.
    """
    return [block for block in fenced_blocks(path) if "systemd-run" in block]


def routine_blocks(path: Path) -> list[str]:
    """Return the routine examples: a named scope running a piped `make`.

    Returns
    -------
    list[str]
        The wrapper blocks that run `make kani-ir` or `make kani-full` through
        `bash -c`.
    """
    return [
        block
        for block in wrapper_blocks(path)
        if "bash -c" in block and ("make kani-ir" in block or "make kani-full" in block)
    ]


def placeholder_blocks(path: Path) -> list[str]:
    """Return the template examples, which leave the command as a placeholder.

    Returns
    -------
    list[str]
        The wrapper blocks that name `<kani-command>` instead of a command.
    """
    return [block for block in wrapper_blocks(path) if "<kani-command>" in block]


def payload_argument(block: str) -> str | None:
    """Return the text between `bash -c` and the closing quote of its argument.

    Only the quoted argument is the payload. Searching the rest of the block
    would also see the outer pipeline and any trailing comment, which is how a
    wrapper can carry `tee` and `pipefail` outside the scope and still look
    conforming.

    Returns
    -------
    str | None
        The quoted argument's text, or ``None`` when the block has no
        `bash -c` or no quote opens its argument.
    """
    marker = "bash -c"
    if marker not in block:
        return None
    remainder = block.split(marker, 1)[1].lstrip()
    quote = remainder[:1]
    if quote not in {"'", '"'}:
        return None
    end = remainder.find(quote, 1)
    if end < 0:
        return None
    return remainder[1:end]


def argument_tokens(block: str) -> list[str]:
    """Return the whitespace-separated tokens of the wrapper's own arguments.

    Everything from `bash -c` onward is the payload, and the payload is where
    an echoed string could otherwise be mistaken for a wrapper option: a block
    whose payload runs `echo "--collect"` would satisfy an option check that
    searched the whole text while the wrapper never collected anything.

    Line continuations are removed first, so a flag split across two source
    lines is still one token.

    Returns
    -------
    list[str]
        The argument tokens before the payload, with backslash-newline joins
        collapsed. Empty when the block is empty.
    """
    arguments = block.split("bash -c", 1)[0]
    # Join continuations first, so a flag split across two source lines is one
    # token, then drop whole-line comments: a comment supplies no arguments, and
    # a block whose comment quotes a bound it never sets must not satisfy a
    # check that reads arguments.
    joined = arguments.replace("\\\n", " ")
    uncommented = [
        line for line in joined.splitlines() if not line.lstrip().startswith("#")
    ]
    return " ".join(uncommented).split()


def property_arguments(block: str) -> dict[str, str]:
    """Return the `-p Name=value` properties a wrapper block actually sets.

    Only the command arguments are read, and everything from `bash -c` onward
    is dropped first: that is the payload, where an echoed string could
    otherwise be mistaken for a wrapper setting. A trailing comment on an
    argument line cannot occur — the block is a continued command, so systemd
    would receive the comment as an argument — which is why cutting at the
    payload is the whole of the protection needed.

    Returns
    -------
    dict[str, str]
        Property name to its raw value, as the last occurrence in argument
        order. Empty when the block sets none.
    """
    properties: dict[str, str] = {}
    words = argument_tokens(block)
    for index, word in enumerate(words):
        value = None
        if word == "-p" and index + 1 < len(words):
            value = words[index + 1]
        elif word.startswith("-p") and len(word) > 2:
            value = word[2:].lstrip("=")
        if value is None:
            continue
        name, _, raw = value.partition("=")
        properties[name] = raw.rstrip("\\")
    return properties


def payload_statements(payload: str) -> list[str]:
    """Return the payload's statements, with `;`-separated ones split out.

    A substring search cannot tell an executed `set -o pipefail` from the same
    text inside a quoted string: `echo "set -o pipefail"` prints the words and
    enables nothing. Splitting on `;` and trimming gives the commands the shell
    would actually run, which is what a contract about *effect* has to inspect.

    This is not a shell parser. It handles the documented payload shape — a
    short `;`-separated preamble followed by one pipeline — and would mis-split
    a `;` inside quotes. That is acceptable here because the documents are ours
    and the check fails closed: a payload it mis-splits simply fails to yield
    the statement the contract wants.

    Returns
    -------
    list[str]
        The trimmed statements, in order, with empty ones dropped.
    """
    return [part.strip() for part in payload.split(";") if part.strip()]
