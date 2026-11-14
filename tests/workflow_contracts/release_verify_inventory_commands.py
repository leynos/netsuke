"""Audit what a step's shell text runs, once its words have been placed.

The verification action's safety rests on the commands it runs being the ones
the contract can read. A ``gh api`` call is inspected for write-implying flags,
the verifier invocation has its tokens pinned, and a token-bearing step must
run only the ``gh`` subcommand the scan has a rule for.

This module owns those checks -- the auditing half of the contract. Reading the
step, by joining its lines and deciding which word bash treats as a command,
lives in ``release_verify_inventory_command_words.py``; the documents the checks
read and the values they pin live in ``release_verify_inventory_support.py``;
the deliberate mutations that show each check notices a change live in
``release_verify_inventory_mutations.py``.

Run via ``make test-workflow-contracts``.
"""

import re
import shlex
import typing as typ

from release_verify_inventory_command_words import (
    COMMAND_SEPARATORS,
    placed_words,
    script_lines,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Long flags that would turn `gh api` into a write: `--field` and
#: `--raw-field` imply POST, and `--input` reads a body from a file. `--method`
#: is checked separately, because the call's own leading one is required.
FORBIDDEN_LONG_FLAGS = ("--field", "--raw-field", "--input")
#: The short spellings of the same flags. `-f` and `-F` imply POST and `-X`
#: sets the method. Matching their first two characters covers the joined
#: forms, such as `-fx=y` and `-XPOST`.
FORBIDDEN_SHORT_FLAGS = ("-f", "-F", "-X")
#: The `gh` subcommand a read-only step may name, and the one that wrote the
#: release it is checking. Any other subcommand is refused: the contract has a
#: rule for `api` alone, because that is the only call the action needs.
READ_ONLY_SUBCOMMAND = "api"
WRITING_SUBCOMMAND = "release"
#: The subcommands the scan can read. A word outside this set -- including one
#: built from a variable, which bash expands before `gh` sees it -- names a
#: call the flag and release checks have no wording to inspect.
READABLE_SUBCOMMANDS = frozenset({READ_ONLY_SUBCOMMAND, WRITING_SUBCOMMAND})
#: The word naming the secrets context inside an expression, whether dotted,
#: indexed, or passed whole to `toJSON`.
SECRET_WORD = re.compile(r"\bsecrets\b")


def _command_tokens(words: list[tuple[str, bool]]) -> list[str]:
    """Return the words of one command, stopping at the operator ending it.

    A line can hold more than one command -- ``gh api ...; gh release delete
    v1`` -- and a flag check that read past the separator would judge the first
    call by the second one's arguments.

    Returns
    -------
    list[str]
        The command's words, with its operators removed.
    """
    tokens: list[str] = []
    for word, _ in words:
        if word in COMMAND_SEPARATORS:
            break
        tokens.append(word)
    return tokens


def gh_invocations(run: str) -> list[list[str]]:
    """Return the shell tokens of every ``gh`` command a step runs.

    The command words come from the shared placement, so a call is found
    wherever bash would read it: at the start of a line, inside a command
    substitution, or behind a wrapper. The tokens are the ones bash hands
    ``gh``, with quotes removed, so ``gh "api"`` and ``gh api`` are the same
    call to every check built on this list.

    Parameters
    ----------
    run
        The step's ``run`` text, as the action manifest carries it.

    Returns
    -------
    list[list[str]]
        Shell tokens for each invocation, in the order the step runs them.
    """
    invocations: list[list[str]] = []
    for line in script_lines(run):
        words = placed_words(line)
        for index, (word, is_command) in enumerate(words):
            if is_command and word == "gh":
                invocations.append(_command_tokens(words[index:]))
    return invocations


def gh_subcommand(tokens: list[str]) -> str:
    """Return the word bash hands ``gh`` as its subcommand, or an empty string.

    Parameters
    ----------
    tokens
        One ``gh`` invocation's shell tokens.

    Returns
    -------
    str
        The subcommand the call names, or ``""`` when it names none.
    """
    return tokens[1] if len(tokens) > 1 else ""


def gh_api_invocations(run: str) -> list[list[str]]:
    """Return the shell tokens of every ``gh api`` invocation in one step.

    The subcommand is read from the tokenized words rather than from the raw
    text, so a quoted spelling cannot present a call the flag checks never
    see: bash removes the quotes before ``gh`` runs, and so does the scan.

    Parameters
    ----------
    run
        The step's ``run`` text, as the action manifest carries it.

    Returns
    -------
    list[list[str]]
        Shell tokens for each invocation, in the order the step runs them.
    """
    return [
        tokens
        for tokens in gh_invocations(run)
        if gh_subcommand(tokens) == READ_ONLY_SUBCOMMAND
    ]


def gh_release_invocations(run: str) -> list[list[str]]:
    """Return the shell tokens of every ``gh release`` invocation in one step.

    Parameters
    ----------
    run
        The step's ``run`` text, as the action manifest carries it.

    Returns
    -------
    list[list[str]]
        Shell tokens for each invocation, in the order the step runs them.
    """
    return [
        tokens
        for tokens in gh_invocations(run)
        if gh_subcommand(tokens) == WRITING_SUBCOMMAND
    ]


def unread_gh_subcommands(run: str) -> list[list[str]]:
    """Return each ``gh`` invocation whose subcommand the scan cannot read.

    Placing a command word says bash runs ``gh``; it does not say which
    subcommand the word after it names. ``subcommand=api; gh "$subcommand"
    --method POST ...`` runs an ``api`` call with the write-implying flags the
    contract refuses, yet the tokens hand it ``$subcommand`` rather than
    ``api``, so the flag check has no call to judge. The same holds for a word
    written out before an expansion, ``gh "a$(printf pi)"``, and for any
    subcommand outside :data:`READABLE_SUBCOMMANDS`, which the contract has no
    rule for.

    Parameters
    ----------
    run
        The step's ``run`` text, as the action manifest carries it.

    Returns
    -------
    list[list[str]]
        The offending invocations' tokens, in the order the step runs them.

    Examples
    --------
    >>> unread_gh_subcommands('sub=api; gh "$sub" --method POST repos/o/r')
    [['gh', '$sub', '--method', 'POST', 'repos/o/r']]
    >>> unread_gh_subcommands('gh api --method GET "repos/$GITHUB_REPOSITORY/x"')
    []
    """
    return [
        tokens
        for tokens in gh_invocations(run)
        if gh_subcommand(tokens) not in READABLE_SUBCOMMANDS
    ]


def forbidden_gh_flags(tokens: list[str]) -> list[str]:
    """Return each argument of one ``gh api`` call that implies a write.

    Matching whole tokens would accept ``--field=x=y`` and ``-fx=y``, so each
    argument is compared by its flag name with any ``=value`` removed. The
    call's own leading ``--method GET`` is the one method override the action
    is allowed to name; any later ``--method`` is a second, unreviewed one.

    Parameters
    ----------
    tokens
        One ``gh api`` invocation's shell tokens.

    Returns
    -------
    list[str]
        The offending arguments, in the order they appear.
    """
    offending: list[str] = []
    # The contract pins `gh api --method GET` as the leading four tokens, so
    # that exact method is the one override the action may name.
    allowed_method = tokens[:4] == ["gh", "api", "--method", "GET"]
    for position, token in enumerate(tokens):
        name = token.split("=", 1)[0]
        if name == "--method":
            if position != 2 or not allowed_method:
                offending.append(token)
        elif name in FORBIDDEN_LONG_FLAGS or name[:2] in FORBIDDEN_SHORT_FLAGS:
            offending.append(token)
    return offending


def run_tokens(run: str) -> list[str]:
    """Return shell tokens from the step's single ``uv run`` invocation.

    The invocation is read as a logical line, so a continuation inside it is
    already joined the way bash joins it and the token list cannot be cut short
    at a backslash. A second invocation is refused rather than ignored: the
    contract pins what the verifier is handed, and a later call could hand it a
    different trusted record after the pinned one had already run.

    Parameters
    ----------
    run
        The step's ``run`` text, as the action manifest carries it.

    Returns
    -------
    list[str]
        The invocation's shell tokens, in the order it passes them.

    Raises
    ------
    AssertionError
        If the step does not run exactly one ``uv run`` command.
    """
    invocations = [shlex.split(line) for line in script_lines(run) if "uv run " in line]
    if len(invocations) != 1:
        msg = f"the step must run exactly one uv command, got {len(invocations)}"
        raise AssertionError(msg)
    return invocations[0]


def expression_bodies(text: str) -> cabc.Iterator[str]:
    """Yield the body of every ``${{ ... }}`` expression in ``text``.

    Looking for the first closing brace stops inside a quoted format string:
    ``format('Hello {0}', secrets.TOKEN)`` closes at the ``0`` and the secret
    that follows is never examined. GitHub expressions quote with single
    quotes only, so a single-quoted run is skipped whole and a brace inside it
    cannot end the expression early.

    Parameters
    ----------
    text
        A string from a workflow or action document, which may carry
        expressions anywhere in it.

    Yields
    ------
    str
        Each expression's text, without its ``${{`` and ``}}`` delimiters.
    """
    start = 0
    while (opening := text.find("${{", start)) != -1:
        quoted = False
        for index in range(opening + 3, len(text)):
            char = text[index]
            if quoted:
                quoted = char != "'"
            elif char == "'":
                quoted = True
            elif text.startswith("}}", index):
                yield text[opening + 3 : index]
                start = index + 2
                break
        else:
            return


def references_secret(text: str) -> bool:
    """Return whether any expression in ``text`` names the secrets context.

    Parameters
    ----------
    text
        A string from a workflow or action document, which may carry
        expressions anywhere in it.

    Returns
    -------
    bool
        ``True`` when an expression names ``secrets``, however it is spelled.
    """
    return any(SECRET_WORD.search(body) for body in expression_bodies(text))


def reject_secrets(node: object) -> None:
    """Fail if any value in a document references a workflow secret.

    The verifier reads only bytes this job downloaded and inputs its caller
    named; a secret reference would be a new credential in that boundary. The
    walk covers every nested value, so placing one inside an untested step
    does not evade it.

    Parameters
    ----------
    node
        A decoded document, or any value inside one. Mappings, lists and
        tuples are walked; strings are examined for expressions.

    Raises
    ------
    AssertionError
        If any string in ``node`` names the secrets context.
    """
    match node:
        case str() as text:
            if references_secret(text):
                msg = f"the verification boundary must not reference a secret: {text!r}"
                raise AssertionError(msg)
        case dict() as mapping:
            for value in mapping.values():
                reject_secrets(value)
        case list() | tuple() as items:
            for item in items:
                reject_secrets(item)
