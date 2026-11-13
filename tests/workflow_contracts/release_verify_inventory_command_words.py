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
import typing as typ

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
#: ``api`` holds. The scan cannot know which of a wrapper's options consume a
#: value -- ``command -p`` takes none, ``xargs -n`` takes the next word -- so
#: both the wrapper's own position and an option's value are read as places a
#: command word can appear, because the safe direction is to refuse a word the
#: scan cannot place.
COMMAND_WRAPPERS = frozenset({
    "builtin",
    "command",
    "env",
    "exec",
    "nohup",
    "time",
    "xargs",
})
#: Commands that run text the scan cannot read: a shell or an interpreter
#: handed a run-time string, and the builtins that evaluate one. No word
#: position model can bound what such a command runs -- ``bash -c "$cmd"``
#: names no command the scan sees, however carefully the wrapper itself is
#: placed -- so the contract refuses the class rather than trying to read the
#: string. The action needs none of them, and its own script file is run by
#: ``uv`` from a pinned path rather than handed to an interpreter by name.
FORBIDDEN_EXECUTORS = frozenset({
    ".",
    "bash",
    "dash",
    "eval",
    "ksh",
    "node",
    "perl",
    "python",
    "python3",
    "ruby",
    "sh",
    "source",
    "zsh",
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


#: The positions where a word can name the command bash ultimately runs: the
#: start of a command, the operand of a wrapper, and the word after a wrapper's
#: option, which is either that option's value or the command. Both checks
#: below read the same positions, so a word hidden at any of them is refused.
_COMMAND_POSITIONS = frozenset({
    _Position.COMMAND,
    _Position.WRAPPED,
    _Position.OPTION_VALUE,
})


def _names_expansion(word: str, position: _Position) -> bool:
    """Return whether ``word`` names a command through a variable expansion.

    Only a word bash reads as a command name qualifies: one that begins a
    command, one that a wrapper will run, or one sitting where a wrapper's
    option expects its value. The last is a command position because the scan
    cannot know which options consume a value: ``command -p`` takes none, so
    ``command -p "$api" api ...`` runs whatever ``api`` holds, while
    ``xargs -n 1 "$api"`` runs it after a value that was consumed. Refusing
    both is the safe direction. The same expansion after a literal command is
    an argument, which a step may legitimately use.

    The expansion may sit anywhere in the word rather than opening it: bash
    joins adjacent quoted and unquoted parts, so ``g"$suffix"`` names ``gh``
    when ``suffix`` holds ``h``, and a check for a leading ``$`` accepts it.
    A quoted ``$`` that bash would not expand is refused too, because the scan
    cannot tell the two apart and refusing is the safe direction.

    A ``NAME=value`` word is an assignment rather than a command, wherever it
    sits: bash stores its value in the variable and runs nothing, so the
    expansion it carries is a value and not a command name. The step's own
    ``trusted_root=$(canonical "$TRUSTED_DIRECTORY")`` is that shape.

    Returns
    -------
    bool
        ``True`` when the word is a command name spelt as an expansion.
    """
    if position not in _COMMAND_POSITIONS:
        return False
    if ENVIRONMENT_ASSIGNMENT.match(word) is not None:
        return False
    return "$" in word


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

    An assignment ahead of a command sets the environment for that command
    rather than becoming the command, so ``EXTRA=1 command "$api" ...`` still
    reaches the wrapper. Keeping the position open is what lets the scan see
    the word the wrapper runs.

    Returns
    -------
    _Position
        The position of the following word.
    """
    if position is _Position.OPTION_VALUE:
        return _Position.WRAPPED
    if position is _Position.WRAPPED:
        return _wrapped_successor(word)
    if position is _Position.COMMAND:
        return _command_successor(word)
    return _Position.ARGUMENT


def _command_successor(word: str) -> _Position:
    """Return the position after a word at the start of a command.

    The word still to come is the command only while the assignments ahead of
    it hold their place; once anything else is written, the command has been
    named and what follows it is an argument.

    Returns
    -------
    _Position
        The position of the following word.
    """
    if ENVIRONMENT_ASSIGNMENT.match(word) is not None:
        return _Position.COMMAND
    if word in COMMAND_WRAPPERS:
        return _Position.WRAPPED
    return _Position.ARGUMENT


def _wrapped_successor(word: str) -> _Position:
    """Return the position of the word after one among a wrapper's operands.

    The scan does not know which of a wrapper's options take a value, so an
    option leaves the next word in option-value position, where an expansion is
    refused rather than trusted, and a ``NAME=value`` assignment that ``env``
    accepts is part of the wrapper too. A second wrapper is still a wrapper,
    because ``command env "$api" ...`` runs what ``env`` runs and the nesting
    has no bound a word count could reach. Anything else is the command the
    wrapper runs.

    Returns
    -------
    _Position
        The position of the following word.
    """
    if word in COMMAND_WRAPPERS:
        return _Position.WRAPPED
    if word.startswith("-"):
        return _Position.OPTION_VALUE
    if ENVIRONMENT_ASSIGNMENT.match(word) is not None:
        return _Position.WRAPPED
    return _Position.ARGUMENT


def _placed_words(line: str) -> list[tuple[str, _Position]]:
    """Return each word on one joined line with the position bash reads it in.

    Both command-word checks ask a different question of the same reading, so
    the traversal is written once and each check supplies its own predicate.

    Returns
    -------
    list[tuple[str, _Position]]
        The line's words and their positions, in order.
    """
    placed: list[tuple[str, _Position]] = []
    position = _Position.COMMAND
    for word in _words(line):
        placed.append((word, position))
        position = _next_position(word, position)
    return placed


def _is_unread_executor(word: str, position: _Position) -> bool:
    """Return whether ``word`` would run text the scan cannot read.

    Returns
    -------
    bool
        ``True`` when the word names a forbidden executor in a command
        position.
    """
    return position in _COMMAND_POSITIONS and word in FORBIDDEN_EXECUTORS


def _command_words(
    run: str, matches: cabc.Callable[[str, _Position], bool]
) -> list[str]:
    """Return every word in a step that ``matches`` accepts, in order.

    Parameters
    ----------
    run
        The step's ``run`` text, as the action manifest carries it.
    matches
        The predicate deciding whether a word, at the position bash reads it
        in, is one the caller is looking for.

    Returns
    -------
    list[str]
        The accepted words, in the order they appear.
    """
    return [
        word
        for line in script_lines(run)
        for word, position in _placed_words(line)
        if matches(word, position)
    ]


def indirect_command_words(run: str) -> list[str]:
    """Return each command word a step builds from a variable expansion.

    A token-bearing step may name an argument with a variable, as the listing
    step does for the release number, but its *commands* must be literal. A
    command word that is an expansion can hide a write behind the literal scan:
    ``api=gh; "$api" api --method POST ...`` executes a POST the scan, looking
    for ``gh api``, never sees. The expansion need not open the word --
    ``suffix=h; g"$suffix" api ...`` builds ``gh`` from a prefix and a variable
    -- and wrapping it in a command that runs its operand hides it the same way,
    ``command "$api" api ...``, so a word in :data:`COMMAND_WRAPPERS` puts the
    word it runs in command position as well, and so does the word after one of
    the wrapper's options, which may be either the option's value or the
    command: ``command -p "$api" api ...`` takes no value and runs ``api``,
    where ``xargs -n 1 "$api"`` does. Wrappers nest, so ``command env "$api"
    api ...`` keeps the wrapped position past ``env`` rather than treating the
    second wrapper as the command. The scan reads bash's own tokenization, so a
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
    return _command_words(run, _names_expansion)


def forbidden_executor_words(run: str) -> list[str]:
    """Return each command word that would run text the scan cannot read.

    Reading a command word answers the question of *where* bash looks for the
    command; it cannot answer what an interpreter is then told to run.
    ``bash -c "$script"``, ``eval "$line"``, ``sh <<<"$body"``, and
    ``python -c "$code"`` all execute text that the step assembles at run time,
    so no amount of position modelling reaches the ``gh api`` call they make.
    :data:`FORBIDDEN_EXECUTORS` names that class, and the check refuses it
    wherever such a command can be written: at the start of a command, as the
    operand of a wrapper, or as the word after a wrapper's option.

    The class is refused wholesale rather than per spelling because the
    spelling is unbounded -- a shell has several ways to feed a string to an
    interpreter, and the action needs none of them. Its own checks stay
    readable: the step runs a pinned script path through ``uv`` and calls
    ``gh``, ``cat``, and ``read`` directly.

    Parameters
    ----------
    run
        The step's ``run`` text, as the action manifest carries it.

    Returns
    -------
    list[str]
        The offending command words, in the order they appear.

    Examples
    --------
    >>> forbidden_executor_words('body=POST; bash -c "$body"')
    ['bash']
    >>> forbidden_executor_words("uv run --no-project script.py")
    []
    """
    return _command_words(run, _is_unread_executor)


if typ.TYPE_CHECKING:
    import collections.abc as cabc
