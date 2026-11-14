"""Audit what a step's shell text runs, once its words have been placed.

The verification action's safety rests on the commands it runs being the ones
the contract can read. A ``gh api`` call is inspected for write-implying flags,
the verifier invocation has its tokens pinned, and a token-bearing step must
name its commands and its ``gh`` subcommands literally.

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

from release_verify_inventory_command_words import script_lines

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
#: A `gh api` call anywhere on a line, including inside a command
#: substitution, which a check anchored at the start of the line would miss.
GH_API_CALL = re.compile(r"\bgh\s+api\b")
#: The `gh release` subcommand, which can write; `gh api` is what a read-only
#: step must use instead.
GH_RELEASE_CALL = re.compile(r"\bgh\s+release\b")
#: A `gh` word and the word bash reads as its subcommand, quoted or bare. Bash
#: expands that word before `gh` sees it, so `gh "$sub"` runs whatever the
#: variable holds and the literal `gh api` scan finds no call to inspect.
GH_SUBCOMMAND = re.compile(r"\bgh\s+(\"[^\"]*\"|'[^']*'|\S+)")
#: The word naming the secrets context inside an expression, whether dotted,
#: indexed, or passed whole to `toJSON`.
SECRET_WORD = re.compile(r"\bsecrets\b")


def gh_api_invocations(run: str) -> list[list[str]]:
    """Return the shell tokens of every ``gh api`` invocation in one step.

    Tokenizing one joined command rather than the whole script keeps the flag
    checks from seeing text that belongs to an unrelated line. The search
    starts wherever the call sits on its line, so a call inside a command
    substitution is scanned like any other.

    Parameters
    ----------
    run
        The step's ``run`` text, as the action manifest carries it.

    Returns
    -------
    list[list[str]]
        Shell tokens for each invocation, in the order the step runs them.

    Raises
    ------
    AssertionError
        If the script names more ``gh api`` calls than the scan tokenizes. An
        unmodelled shape must fail the contract rather than slip past it.
    """
    script = script_lines(run)
    invocations = [
        shlex.split(line[call.start() :])
        for line in script
        if (call := GH_API_CALL.search(line)) is not None
    ]
    mentioned = len(GH_API_CALL.findall("\n".join(script)))
    if mentioned != len(invocations):
        msg = (
            f"every gh api call must be tokenized, found {mentioned} call(s) "
            f"and {len(invocations)} invocation(s)"
        )
        raise AssertionError(msg)
    return invocations


def unread_subcommand_lines(run: str) -> list[str]:
    """Return each line whose ``gh`` subcommand the scan cannot read.

    Placing a command word says bash runs ``gh``; it does not say which
    subcommand the word after it names. ``subcommand=api; gh "$subcommand"
    --method POST ...`` runs an ``api`` call with the write-implying flags the
    contract refuses, yet the literal ``gh api`` scan finds nothing: the word it
    looks for is the one bash builds from the variable. A word written out
    before the expansion -- ``gh "a$(printf pi)"`` -- escapes the literal scan
    the same way, and a quoted ``$`` is refused because the scan cannot tell it
    from one bash expands.

    Parameters
    ----------
    run
        The step's ``run`` text, as the action manifest carries it.

    Returns
    -------
    list[str]
        The offending joined lines, in the order the step runs them.

    Examples
    --------
    >>> unread_subcommand_lines('sub=api; gh "$sub" --method POST repos/o/r')
    ['sub=api; gh "$sub" --method POST repos/o/r']
    >>> unread_subcommand_lines('gh api --method GET "repos/$GITHUB_REPOSITORY/x"')
    []
    """
    return [
        line
        for line in script_lines(run)
        if any("$" in word for word in GH_SUBCOMMAND.findall(line))
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
