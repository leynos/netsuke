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

#: One fenced block, capturing its language tag and body.
#:
#: The fence may be indented: the 4.2.2 plan carries its template inside a list
#: item, so requiring a fence at column zero would silently skip it.
FENCE = re.compile(
    r"^[ \t]*```(?P<language>[\w+-]*)\n(?P<body>.*?)^[ \t]*```",
    re.MULTILINE | re.DOTALL,
)


def fenced_blocks(path: Path) -> list[str]:
    """Return the body of every fenced code block in ``path``.

    Returns
    -------
    list[str]
        The text between each pair of fences, in document order.
    """
    return [
        match.group("body")
        for match in FENCE.finditer(path.read_text(encoding="utf-8"))
    ]


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
    arguments = block.split("bash -c", 1)[0]
    properties: dict[str, str] = {}
    words = arguments.split()
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
