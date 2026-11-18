"""Read a smoke step's script as bash runs it, not as one line of it.

The reviewed invocation is one command inside a step's ``run`` block, so an
exact match on that line says nothing about the script around it. ``exit 0``
above it makes it unreachable, ``if false; then`` above it confines it to a
branch bash never takes, and ``set +e`` above it lets a refusal fall through to
an ``exit 0`` below. The readers here answer those questions, and only from the
executable lines, so a comment naming a construct cannot stand in for one.

Reachability and failure propagation are read as a shape rather than as a
search for one suspicious line. A step may prepare the invocation with
``set -…`` and plain assignments and nothing else; every other construct is
refused rather than assumed harmless, because a construct nobody reviewed is
exactly what a contract must not wave through.

The invocation itself is located by its shell words, not by the text of a line.
An assignment whose value happens to spell the CLI's path is a prologue line,
not the invocation, so it neither ends the search for the command nor hides a
construct that follows it.

Which command the step must run is asserted in
``release_publish_path_consumer_commands``, which composes these readers.

Run via ``make test-workflow-contracts``.
"""

import dataclasses
import re
import shlex

from release_publish_path_consumer_shape import SMOKE_CLI, command_lines

#: The shape of one assignment: a shell name, ``=``, then a value carrying no
#: separator and no backtick. The shape alone accepts ``NAME=1 exit 0``, which
#: bash runs as ``exit 0``, so a line must also tokenize to this one word.
_ASSIGNMENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=[^;&|`]*")
#: A substitution that runs a command. A plain ``$NAME`` expands a value and is
#: what the shipped prologue uses; ``$(…)`` runs something of its own.
_COMMAND_SUBSTITUTION = re.compile(r"\$\(")
#: The ``set`` letters a prologue may turn on: ``-e`` fails fast and ``-u``
#: refuses an unset variable. Every other letter either disables execution
#: (``-n``) or changes how the invocation's words are expanded.
_REVIEWED_SET_LETTERS = frozenset("eu")
#: The one ``set -o`` name a prologue may turn on.
_REVIEWED_SET_OPTION = "pipefail"


@dataclasses.dataclass(frozen=True, slots=True)
class Invocation:
    """Hold one parsed boundary between a script's prologue and its command.

    The three readings answer different questions and must come from the same
    line: the position slices the script around the command, the words are what
    the contract compares, and the text is what quoting checks read. Deriving
    them separately would let a check compare one line's words against
    another's text.
    """

    index: int
    tokens: tuple[str, ...]
    line: str


def ends_successfully(line: str) -> bool:
    """Return whether one shell command line exits the step successfully.

    Returns
    -------
    bool
        True for a bare ``exit`` or ``return``, or one naming status ``0``.
        A non-zero status fails the step instead of skipping the rest of it,
        so it makes no command unreachable.

    Examples
    --------
    >>> ends_successfully("exit 0"), ends_successfully("exit 1")
    (True, False)
    """
    tokens = _tokenised(line)
    return (
        tokens is not None
        and tokens[:1] in {("exit",), ("return",)}
        and (tokens[1:] in {(), ("0",)})
    )


def _tokenised(line: str) -> tuple[str, ...] | None:
    """Return the line's shell words, or ``None`` when it cannot be tokenized."""
    try:
        return tuple(shlex.split(line))
    except ValueError:
        return None


def invocation(step: dict[str, object]) -> Invocation | None:
    """Return the parsed boundary around the line that runs the smoke CLI.

    Returns
    -------
    Invocation | None
        Where the invocation sits among the step's executable lines, its shell
        words including the launcher, and the text those words came from, or
        ``None`` when no line runs :data:`SMOKE_CLI`.

    Notes
    -----
    The line is found by its words rather than by the text of the line, so an
    assignment whose value spells the CLI's path is not mistaken for the
    invocation and does not end the search for the command.
    """
    return _found_in(command_lines(step.get("run")))


def runs_smoke_cli(line: str) -> bool:
    """Return whether one command line runs :data:`SMOKE_CLI`.

    Returns
    -------
    bool
        True when the line's shell words include :data:`SMOKE_CLI`. Matching on
        the words is what keeps an assignment whose value spells the CLI's path
        from reading as an invocation.

    Examples
    --------
    >>> runs_smoke_cli("uv run scripts/consumer_smoke_cli.py verify")
    True
    >>> runs_smoke_cli('script="scripts/consumer_smoke_cli.py"')
    False
    """
    tokens = _tokenised(line)
    return tokens is not None and SMOKE_CLI in tokens


def hides_a_reference(line: str) -> bool:
    """Return whether the line single-quotes a ``$``, which bash will not expand.

    Returns
    -------
    bool
        True when a ``$`` sits inside single quotes. Tokenising cannot show
        this, because the quotes are removed: ``"$IDENTITY_PATH"`` and
        ``'$IDENTITY_PATH'`` produce the same word, and only the second reaches
        the program as the literal text.

    Notes
    -----
    A quote inside the other kind of quote is literal, so the scan tracks both
    states rather than toggling on every quote character.

    Examples
    --------
    >>> hides_a_reference('--identity "$IDENTITY_PATH"')
    False
    >>> hides_a_reference("--identity '$IDENTITY_PATH'")
    True
    """
    single = False
    double = False
    for character in line:
        if character == "'" and not double:
            single = not single
        elif character == '"' and not single:
            double = not double
        elif character == "$" and single:
            return True
    return False


def _found_in(lines: list[str]) -> Invocation | None:
    """Return the first line of ``lines`` that runs :data:`SMOKE_CLI`."""
    for index, line in enumerate(lines):
        tokens = _tokenised(line)
        if tokens is not None and SMOKE_CLI in tokens:
            return Invocation(index=index, tokens=tokens, line=line)
    return None


def smoke_command_line(step: dict[str, object]) -> tuple[str, ...] | None:
    """Return the complete command line a step runs for the smoke CLI.

    Returns
    -------
    tuple[str, ...] | None
        Every token of the first line that runs :data:`SMOKE_CLI`, launcher
        included, or ``None`` when no line does.
    """
    found = invocation(step)
    return None if found is None else found.tokens


def invoked_phase(tokens: tuple[str, ...] | None) -> str | None:
    """Return the subcommand one tokenised command line runs, or ``None``.

    Returns
    -------
    str | None
        The token after :data:`SMOKE_CLI`, which names the phase the CLI is
        asked to run, or ``None`` when the line does not run the CLI or names
        no phase after it.
    """
    if tokens is None:
        return None
    for index, token in enumerate(tokens):
        if token == SMOKE_CLI:
            return tokens[index + 1] if index + 1 < len(tokens) else None
    return None


def _is_assignment(line: str) -> bool:
    """Return whether one line assigns one variable and runs nothing else.

    Returns
    -------
    bool
        True when the line tokenizes to exactly one word of the assignment's
        shape. Tokenizing is what refuses ``NAME=1 exit 0``, which the shape
        alone accepts and bash runs as ``exit 0``, and the substitution check
        refuses a value that runs a command of its own.
    """
    if _COMMAND_SUBSTITUTION.search(line):
        return False
    tokens = _tokenised(line)
    return (
        tokens is not None
        and len(tokens) == 1
        and _ASSIGNMENT.fullmatch(tokens[0]) is not None
    )


def _is_enabling_set(line: str) -> bool:
    """Return whether one line only turns on the reviewed ``set`` options.

    Returns
    -------
    bool
        True when every option is a reviewed enabling option. ``set -n`` skips
        execution, ``set +e`` clears fail-fast, and an unreviewed ``-o`` name
        changes what the invocation's own words mean; none of those is a
        prologue a reviewed step carries.
    """
    tokens = _tokenised(line)
    return (
        tokens is not None
        and bool(tokens)
        and tokens[0] == "set"
        and _enables_only_reviewed(tokens[1:])
    )


def _flag_letters(option: str) -> str | None:
    """Return one argument's option letters, or ``None`` when it is not a flag.

    Returns
    -------
    str | None
        The letters after the leading ``-``, or ``None`` for an argument that
        is not a flag at all, such as a bare option name.
    """
    if not option.startswith("-") or len(option) < 2:
        return None
    return option[1:]


def _without_set_option(remaining: list[str], letters: str) -> str | None:
    """Consume a trailing ``-o``'s argument name and return the letters left.

    Returns
    -------
    str | None
        The letters minus the trailing ``o``, or ``None`` when ``-o`` is not
        last or the name it takes is not the reviewed one. Consuming the name
        here is what lets a combined flag such as ``-euo pipefail`` be read as
        one argument list rather than a pattern.

    Notes
    -----
    The name is only consumed once the option is known to take one, so a
    refused list leaves nothing half-read for the caller to reason about.
    """
    if "o" not in letters:
        return letters
    if letters[-1] != "o" or not remaining:
        return None
    if remaining.pop(0) != _REVIEWED_SET_OPTION:
        return None
    return letters[:-1]


def _enables_only_reviewed(options: tuple[str, ...]) -> bool:
    """Return whether a ``set`` argument list only enables reviewed options.

    Returns
    -------
    bool
        True when every argument turns one on and no argument is left over.
    """
    remaining = list(options)
    if not remaining:
        return False
    while remaining:
        letters = _flag_letters(remaining.pop(0))
        if letters is None:
            return False
        letters = _without_set_option(remaining, letters)
        if letters is None or not set(letters) <= _REVIEWED_SET_LETTERS:
            return False
    return True


def _is_prologue(line: str) -> bool:
    """Return whether one line only prepares the step for the invocation.

    Returns
    -------
    bool
        True for an assignment of one variable or a ``set`` that turns on the
        reviewed options, which are the two things a step needs before the
        command it exists to run. Everything else — a conditional, a loop, a
        bare command, or a ``set +e`` — can decide whether the invocation runs
        at all.
    """
    return _is_assignment(line) or _is_enabling_set(line)


def unsupported_prologue(step: dict[str, object]) -> str | None:
    r"""Return the first line before the CLI that could skip or disarm it.

    The reviewed invocation is one command inside a script, so an exact match
    on that line says nothing about the script around it. ``exit 0`` above it
    makes it unreachable, ``if false; then`` above it confines it to a branch
    bash never takes, and ``set +e`` above it lets a refusal fall through to an
    ``exit 0`` below. Only a prologue that plainly prepares the step is
    accepted, so a construct nobody reviewed is refused rather than assumed
    harmless.

    Returns
    -------
    str | None
        The first line ahead of the invocation that is not a plain prologue, or
        ``None`` when every line ahead of it prepares the step.

    Examples
    --------
    >>> step = {"run": "set -e\nexit 0\nuv run x scripts/consumer_smoke_cli.py"}
    >>> unsupported_prologue(step)
    'exit 0'
    """
    lines = command_lines(step.get("run"))
    found = _found_in(lines)
    if found is None:
        return None
    return next(
        (line for line in lines[: found.index] if not _is_prologue(line)),
        None,
    )


def ends_after_smoke_cli(step: dict[str, object]) -> str | None:
    """Return the line that ends a step successfully after the CLI runs.

    A success exit below the invocation absorbs the refusal: with the shell's
    fail-fast disabled, or never enabled, a verifier that exits non-zero is
    followed by ``exit 0`` and the step reports success.

    Returns
    -------
    str | None
        The first success exit that follows the invocation, or ``None`` when
        nothing after it would end the step successfully.
    """
    lines = command_lines(step.get("run"))
    found = _found_in(lines)
    if found is None:
        return None
    return next(
        (line for line in lines[found.index + 1 :] if ends_successfully(line)),
        None,
    )


__all__ = [
    "Invocation",
    "ends_after_smoke_cli",
    "ends_successfully",
    "hides_a_reference",
    "invocation",
    "invoked_phase",
    "runs_smoke_cli",
    "smoke_command_line",
    "unsupported_prologue",
]
