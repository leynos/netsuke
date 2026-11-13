"""Read a step's shell text as bash reads it, and place every word.

The verification action's safety rests on the commands it runs being the ones
the contract can read. A ``gh api`` call is inspected for write-implying flags,
the verifier invocation has its tokens pinned, and a token-bearing step must
name its commands literally. Each check starts from the step's ``run`` text, and
a scan of the raw lines misses three shapes bash does not: a call continued
across a backslash-newline, a call inside a command substitution, and a command
word built from a variable that an earlier assignment set -- directly, or as the
operand of a command that runs it.

This module owns that reading -- the joined logical lines, their words, and
which of those words bash treats as naming a command. The checks built on it
live in ``release_verify_inventory_commands.py``; the documents they read and
the values they pin live in ``release_verify_inventory_support.py``.

Run via ``make test-workflow-contracts``.
"""

import enum
import re
import shlex

import release_verify_inventory_shell as shell

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
#: Commands that run the word after them rather than doing work themselves, so
#: that word names a command too: ``command "$api" api ...`` executes whatever
#: ``api`` holds. The rest of a wrapped command is read as command position,
#: without modelling which options consume a value, because the safe direction
#: is to refuse a word the scan cannot place.
COMMAND_WRAPPERS = frozenset({
    "builtin",
    "command",
    "env",
    "exec",
    "nohup",
    "time",
    "xargs",
})
#: The ``NAME=value`` word ``env`` accepts before the command it runs.
ENVIRONMENT_ASSIGNMENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=")


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


class _Position(enum.Enum):
    """Where bash reads the next word of a command line.

    The scan follows a line one word at a time, and what a word means depends
    on what bash expects there: a command name, the operand of a wrapper that
    runs it, the value of a wrapper's option, an argument, or an operand inside
    a ``[[ ... ]]`` expression.
    """

    COMMAND = enum.auto()
    WRAPPED = enum.auto()
    OPTION_VALUE = enum.auto()
    ARGUMENT = enum.auto()
    TEST = enum.auto()


def _names_expansion(word: str, position: _Position) -> bool:
    """Return whether ``word`` names a command through a variable expansion.

    Only a word bash reads as a command name qualifies: one that begins a
    command, or one that a wrapper will run. The same ``$``-leading word after
    a literal command is an argument, which a step may legitimately use.

    Returns
    -------
    bool
        ``True`` when the word is a command name spelt as an expansion.
    """
    if position not in {_Position.COMMAND, _Position.WRAPPED}:
        return False
    return word.startswith("$")


def _next_position(word: str, position: _Position) -> _Position:
    """Return where bash reads the word after ``word``.

    Returns
    -------
    _Position
        The position of the following word.
    """
    if position is _Position.TEST:
        return _Position.ARGUMENT if word == TEST_CLOSE else _Position.TEST
    if word == TEST_OPEN:
        return _Position.TEST
    if word in COMMAND_SEPARATORS:
        return _Position.COMMAND
    return _after_plain_word(word, position)


def _after_plain_word(word: str, position: _Position) -> _Position:
    """Return the position after a word that is neither a test nor an operator.

    Returns
    -------
    _Position
        The position of the following word.
    """
    if position is _Position.OPTION_VALUE:
        return _Position.WRAPPED
    if position is _Position.WRAPPED:
        return _wrapped_successor(word)
    if position is _Position.COMMAND and word in COMMAND_WRAPPERS:
        return _Position.WRAPPED
    return _Position.ARGUMENT


def _wrapped_successor(word: str) -> _Position:
    """Return the position of the word after one among a wrapper's operands.

    The scan does not know which of a wrapper's options take a value, so an
    option leaves its own value to be read as part of the wrapper, and a
    ``NAME=value`` assignment that ``env`` accepts is part of it too. Anything
    else is the command the wrapper runs.

    Returns
    -------
    _Position
        The position of the following word.
    """
    if word.startswith("-"):
        return _Position.OPTION_VALUE
    if ENVIRONMENT_ASSIGNMENT.match(word) is not None:
        return _Position.WRAPPED
    return _Position.ARGUMENT


def _indirect_words(line: str) -> list[str]:
    """Return the words in one joined line that name a command by expansion.

    Returns
    -------
    list[str]
        The offending words, in the order they appear.
    """
    position = _Position.COMMAND
    indirect: list[str] = []
    for word in _words(line):
        if _names_expansion(word, position):
            indirect.append(word)
        position = _next_position(word, position)
    return indirect


def indirect_command_words(run: str) -> list[str]:
    """Return each command word a step builds from a variable expansion.

    A token-bearing step may name an argument with a variable, as the listing
    step does for the release number, but its *commands* must be literal. A
    command word that is an expansion can hide a write behind the literal scan:
    ``api=gh; "$api" api --method POST ...`` executes a POST the scan, looking
    for ``gh api``, never sees. Wrapping the expansion in a command that runs
    its operand hides it the same way -- ``command "$api" api ...`` -- so a
    word in :data:`COMMAND_WRAPPERS` puts the word it runs in command position
    as well. The scan reads bash's own tokenization, so a variable used as an
    operand -- ``[[ -n "$value" ]]`` -- is not mistaken for one.

    Parameters
    ----------
    run
        The step's ``run`` text, as the action manifest carries it.

    Returns
    -------
    list[str]
        The offending command words, in the order they appear.
    """
    return [word for line in script_lines(run) for word in _indirect_words(line)]
