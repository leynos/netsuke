"""Read a step's shell text as bash reads it, and audit what it runs.

The verification action's safety rests on the commands it runs being the ones
the contract can read. A ``gh api`` call is inspected for write-implying flags,
the verifier invocation has its tokens pinned, and a token-bearing step must
name its commands literally. Each check starts from the step's ``run`` text, and
a scan of the raw lines misses three shapes bash does not: a call continued
across a backslash-newline, a call inside a command substitution, and a command
word built from a variable that an earlier assignment set.

This module owns the joining and tokenizing those checks share -- the reading
side of the contract. The documents the checks read and the values they pin
live in ``release_verify_inventory_support.py``; the deliberate mutations that
show each check notices a change live in
``release_verify_inventory_mutations.py``.

Run via ``make test-workflow-contracts``.
"""

import re
import shlex
import typing as typ

import release_verify_inventory_shell as shell

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
#: The word naming the secrets context inside an expression, whether dotted,
#: indexed, or passed whole to `toJSON`.
SECRET_WORD = re.compile(r"\bsecrets\b")
#: The operator tokens after which bash starts a new command, so the word that
#: follows is a command word rather than an argument. The keywords end a
#: compound command's header, and a subshell, a group, or a negated pipeline
#: begins after its opening token.
COMMAND_SEPARATORS = frozenset({
    ";",
    ";;",
    "&&",
    "||",
    "|",
    "&",
    "(",
    ")",
    "{",
    "}",
    "!",
    "then",
    "do",
    "else",
    "elif",
})
#: The conditional-expression delimiters. Between them `||`, `&&` and `(` are
#: test operators rather than command separators, so a bare `$1` there is an
#: operand and must not be read as a command word.
TEST_OPEN = "[["
TEST_CLOSE = "]]"


def script_lines(run: str) -> list[str]:
    r"""Return a step's shell commands as bash reads them, one per line.

    A backslash-newline is deleted before bash parses, so ``gh a\\`` on one
    line and ``pi`` on the next are one ``gh api`` call -- and a scan of the raw
    lines sees neither. The quoting and escape rules that make the deletion
    conditional live in ``release_verify_inventory_shell``.

    Parameters
    ----------
    run
        The step's ``run`` text, as the action manifest carries it.

    Returns
    -------
    list[str]
        The joined commands, in the order the step runs them.
    """
    return shell.logical_lines(run)


def _words(line: str) -> list[str]:
    """Split one joined line into the words and operators bash separates.

    Returns
    -------
    list[str]
        The line's words, with each control operator as its own token.

    Raises
    ------
    AssertionError
        If the line uses a quoting shape this tokenizer cannot resolve, such
        as a double quote inside a command substitution that it reads as
        closing the enclosing one. Refusing is the safe direction: the scan
        then cannot say which word is a command word, and a step it cannot
        read must not pass the read-only contract on trust.
    """
    lexer = shlex.shlex(line, posix=True, punctuation_chars=True)
    lexer.whitespace_split = True
    try:
        return list(lexer)
    except ValueError as error:
        msg = f"cannot split step line into words ({error}): {line!r}"
        raise AssertionError(msg) from error


def indirect_command_words(run: str) -> list[str]:
    """Return each command word a step builds from a variable expansion.

    A token-bearing step may name an argument with a variable, as the listing
    step does for the release number, but its *commands* must be literal. A
    command word that is an expansion can hide a write behind the literal scan:
    ``api=gh; "$api" api --method POST ...`` executes a POST the scan, looking
    for ``gh api``, never sees. The scan reads bash's own tokenization, so a
    variable used as an operand -- ``[[ -n "$value" ]]`` -- is not mistaken for
    one.

    Parameters
    ----------
    run
        The step's ``run`` text, as the action manifest carries it.

    Returns
    -------
    list[str]
        The offending command words, in the order they appear.
    """
    indirect: list[str] = []
    for line in script_lines(run):
        starts_command = True
        in_test = False
        for word in _words(line):
            if word == TEST_OPEN:
                in_test = True
            elif word == TEST_CLOSE:
                in_test = False
            elif starts_command and word.startswith("$"):
                indirect.append(word)
            starts_command = not in_test and word in COMMAND_SEPARATORS
    return indirect


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
