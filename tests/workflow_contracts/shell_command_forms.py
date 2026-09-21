"""Generated shell forms, and a reference model for what they should read as.

The example-based suite in ``shell_command_scan_test`` states one spelling at a
time what each form reads as. This module supplies the other half: a plan for a
script, built up out of the grammar the scanners claim to support, carrying
along what that plan says each scanner function should return. Because the
expectation is recorded when the script is *built* rather than recovered by
parsing it again, a test consuming this module compares the scanner against an
independent account of the script rather than against a second reading of the
same text.

No shell runs here or in the tests that use this module, and none is wanted.
The oracle is the plan: a generated script never has to be something a shell
would accept, and the properties are stated over what the scanners should read
rather than over what a host happens to do with the text — which also keeps the
suite free of the platform differences a real shell would introduce.

The generators are bounded by construction rather than by ``max_examples``
alone. Every word is drawn from a short alphabet that excludes whitespace, the
shell separators, and the quoting characters, so a generated script is a script
about the forms under test and not about how a shell would quote or split
something exotic.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import dataclasses as dc
import typing as typ

from hypothesis import strategies as st

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The commands a generated script may invoke.
#:
#: Deliberately small and closed rather than drawn from a word list. Several
#: properties ask what a scanner returns for a command the script did *not*
#: run, and that question is only meaningful over a vocabulary the test knows
#: the whole of.
COMMAND_NAMES: typ.Final[tuple[str, ...]] = (
    "cp",
    "install",
    "mkdir",
    "mktemp",
    "echo",
    "ls",
)

#: The command whose directory-creating output a script may capture.
CAPTURE_COMMAND: typ.Final[str] = "mktemp"

#: The flag a capturing `mktemp` call carries.
CAPTURE_FLAG: typ.Final[str] = "-d"

#: Paths a command may be named through, before its own name.
PATH_PREFIXES: typ.Final[tuple[str, ...]] = ("", "/usr/bin/", "/bin/", "./")

#: The operators a script may join its commands with.
#:
#: Both spacing conventions are present: a lane writes `&&` against the words
#: as often as it writes it spaced, and the split has to hold either way.
SEPARATORS: typ.Final[tuple[str, ...]] = ("&&", "||", ";", "|", " && ")

#: A name an assignment may bind.
ASSIGNMENT_NAMES: typ.Final[st.SearchStrategy[str]] = st.from_regex(
    r"[A-Za-z_][A-Za-z0-9_]{0,6}", fullmatch=True
)

#: The literal values an assignment may carry.
#:
#: One value contains a `/`, so the prefix matching that a path-shaped word
#: exercises is reached from the assignment side as well as from the command
#: side. None contains whitespace or a separator, because those would change
#: where the script's own boundaries fall rather than test them.
LITERAL_VALUES: typ.Final[st.SearchStrategy[str]] = st.sampled_from((
    "staging",
    "build/out",
    "1",
    "${ROOT}",
))

#: The commands a mention form runs, as opposed to the one it names.
#:
#: A mention is written by *printing* it: `echo` and `printf` are how a script
#: puts a command's words on the screen without running them. A property asks
#: that a mention reads as exactly one of these and never as the command it
#: named, so the pair is named here rather than spelled inside the forms.
PRINT_COMMANDS: typ.Final[tuple[str, ...]] = ("echo", "printf")

#: A command name a generated mention may name.
#:
#: Excludes the print commands, because a form that printed its own name would
#: be an invocation rather than a mention and the two cases would be the same
#: input read two ways.
MENTIONABLE_COMMANDS: typ.Final[tuple[str, ...]] = tuple(
    name for name in COMMAND_NAMES if name not in PRINT_COMMANDS
)

#: An operand word that carries no meaning to the scanner.
#:
#: The alphabet excludes `/` on purpose. A word containing `/` spells a
#: path-prefixed command name — `a/cp` — and the prefix `\S*/` admits a `/` in
#: the operand position, so the word after the path is read as a command with
#: the earlier words as its operands. That over-approximation is a real
#: property of the scanner, but it is a property about path prefixes rather
#: than about operands, and generating it here would make every operand
#: property fail for a reason that has nothing to do with operands. It has its
#: own case instead, which writes the path-shaped word where a command is read
#: from and after a command's own words.
PLAIN_OPERANDS: typ.Final[st.SearchStrategy[str]] = st.from_regex(
    r"[A-Za-z0-9_][A-Za-z0-9_.]{0,8}", fullmatch=True
)

#: An operand word beginning with a hyphen, which the scanner drops.
#:
#: `command_operands` returns the words a command is *handed whose meaning it
#: reads*, and a flag is not one of them: `mkdir --parents x` yields `x`. The
#: two alphabets are separate because a flag is an operand for the purpose of
#: writing a segment and is not one for the purpose of reading it, so a plan
#: has to say which of the two it drew.
FLAG_OPERANDS: typ.Final[st.SearchStrategy[str]] = st.from_regex(
    r"-{1,2}[A-Za-z][A-Za-z0-9-]{0,6}", fullmatch=True
)

#: An operand word, flag or plain.
OPERAND_WORDS: typ.Final[st.SearchStrategy[str]] = st.one_of(
    PLAIN_OPERANDS, FLAG_OPERANDS
)


@dc.dataclass(frozen=True)
class Assignment:
    """One leading `VAR=value` word, and what its value is.

    ``captures`` records that the value is a command substitution running
    :data:`CAPTURE_COMMAND` — the form whose assigned name holds a directory a
    later command was given.
    """

    #: The name the assignment binds.
    name: str
    #: The literal the value carries, when it is not a capture.
    literal: str
    #: Whether the value is a command substitution rather than a literal.
    captures: bool

    def render(self) -> str:
        """Return the assignment as a script writes it."""
        if self.captures:
            return f'{self.name}="$({CAPTURE_COMMAND} {CAPTURE_FLAG})"'
        return f"{self.name}={self.literal}"


@dc.dataclass(frozen=True)
class Operands:
    """A command's operand words, as written and as read.

    Two lists rather than one, because the scanner drops flag words: a plan
    that recorded only the words it wrote could not say what
    :func:`command_operands` should return for them, and a plan that recorded
    only the words it expects would have to guess what a flag looks like — the
    guess the scanner's own behaviour is supposed to be the subject of.
    """

    #: Every word written after the command name, in order.
    written: tuple[str, ...] = ()
    #: The words the scanner returns, which is ``written`` less its flags.
    read: tuple[str, ...] = ()

    @classmethod
    def of(cls, written: cabc.Sequence[str]) -> Operands:
        """Return the plan for a list of written words.

        Parameters
        ----------
        written
            The words, in the order a script writes them.

        Returns
        -------
        Operands
            The words, with the reads derived the way the scanner derives them.
        """
        return cls(
            tuple(written),
            tuple(word for word in written if not word.startswith("-")),
        )


def operands() -> st.SearchStrategy[Operands]:
    """Return a strategy for a command's operand words.

    Returns
    -------
    st.SearchStrategy
        Strategies for up to four operands, flag and plain mixed.
    """
    return st.lists(OPERAND_WORDS, max_size=4).map(Operands.of)


def assignments() -> st.SearchStrategy[tuple[Assignment, ...]]:
    """Return a strategy for the leading assignments of one command.

    Names are unique within one command's head. The shell permits a repeated
    name — the last wins — but the scanner reports a *set* of captured names,
    so a repetition could only ever be an input where the expectation and the
    reading agree by way of two different collapses. Held unique, the two
    collapses are the same one.

    Returns
    -------
    st.SearchStrategy
        Strategies for tuples of zero to three assignments.
    """
    return st.lists(
        st.builds(
            Assignment,
            name=ASSIGNMENT_NAMES,
            literal=LITERAL_VALUES,
            captures=st.booleans(),
        ),
        max_size=3,
        unique_by=lambda assignment: assignment.name,
    ).map(tuple)


#: A single simple command, and how it is written.
SINGLE_COMMANDS: typ.Final[st.SearchStrategy[Invocation]] = st.builds(
    lambda command, operands, prefix, head: Invocation(
        command=command,
        operands=operands,
        path_prefix=prefix,
        assignments=head,
    ),
    command=st.sampled_from(COMMAND_NAMES),
    operands=operands(),
    prefix=st.sampled_from(PATH_PREFIXES),
    head=assignments(),
)


@dc.dataclass(frozen=True)
class Invocation:
    """One simple command, and everything written around it.

    The fields are what the scanner is expected to recover, so a test asks the
    plan rather than a second reading of the rendered text.
    """

    #: The command the segment runs, without its path.
    command: str
    #: The operand words, as written and as read.
    operands: Operands = Operands()
    #: The path the command is named through, ending in `/`.
    path_prefix: str = ""
    #: The `VAR=value` words written before the command.
    assignments: tuple[Assignment, ...] = ()

    def words(self) -> list[str]:
        """Return the segment's words, in the order a script writes them."""
        return [
            *(assignment.render() for assignment in self.assignments),
            f"{self.path_prefix}{self.command}",
            *self.operands.written,
        ]

    def render(self) -> str:
        """Return the segment as a script writes it on one line."""
        return " ".join(self.words())

    def continued(self, split_after: int) -> str:
        """Return the segment with a backslash-newline after ``split_after``.

        A split point at either end leaves the segment as :meth:`render`
        writes it, which is the identity a continuation is supposed to be: the
        shell removes the backslash and the newline before it parses the
        command, so every split point must read the same as no split at all.

        Parameters
        ----------
        split_after
            How many words to write before the continuation.

        Returns
        -------
        str
            The segment, continued or not.
        """
        words = self.words()
        head = " ".join(words[:split_after])
        tail = " ".join(words[split_after:])
        if not head or not tail:
            return self.render()
        return f"{head} \\\n {tail}"

    def captured_names(self) -> set[str]:
        """Return the names whose value captures the directory command."""
        return {
            assignment.name for assignment in self.assignments if assignment.captures
        }

    def other_commands(self) -> list[str]:
        """Return the vocabulary's command names other than this one."""
        return [name for name in COMMAND_NAMES if name != self.command]


@dc.dataclass(frozen=True)
class Script:
    """Several invocations, and the operators joining them."""

    #: The commands the script runs, in order.
    invocations: tuple[Invocation, ...] = ()
    #: The operator written between each pair of them.
    separators: tuple[str, ...] = ()

    def render(self) -> str:
        """Return the script as a single line a lane could write."""
        parts: list[str] = []
        for index, invocation in enumerate(self.invocations):
            if index:
                parts.extend((self.separators[index - 1], " "))
            parts.append(invocation.render())
        return "".join(parts)


def scripts() -> st.SearchStrategy[Script]:
    """Return a strategy for scripts joining one to four commands.

    The operators are drawn first and the commands sized to match, because a
    script of *n* commands has exactly *n - 1* operators between them. Drawing
    the two independently and discarding the mismatch would bias the generator
    towards the lengths that survive the filter.

    Returns
    -------
    st.SearchStrategy
        Strategies for scripts.
    """

    @st.composite
    def build(draw: st.DrawFn) -> Script:
        """Draw a script whose operators and commands agree in number."""
        separators = draw(st.lists(st.sampled_from(SEPARATORS), max_size=3))
        count = len(separators) + 1
        invocations = draw(st.lists(SINGLE_COMMANDS, min_size=count, max_size=count))
        return Script(tuple(invocations), tuple(separators))

    return build()


def mention_forms(
    command: str, operands: tuple[str, ...]
) -> tuple[tuple[str, tuple[str, ...]], ...]:
    """Return scripts that name ``command`` without running it.

    Each form is a way a script refers to a command it does not execute: it
    prints the words, quotes them, or writes them in a comment. The scanner
    reads a command name where a command *runs* — after the segment's leading
    whitespace, its assignments, and its path — so none of these is an
    invocation, and a rule that read one would certify work the script never
    did.

    Each form is paired with the commands it does run, which is what makes the
    property stateable: a mention reads as one of *those* and never as the one
    it named. The pair is returned rather than left to the caller to infer,
    because the caller's obvious guess — the command being mentioned — is the
    one answer that is always wrong.

    Parameters
    ----------
    command
        The command name being mentioned. Must be one a generated script may
        name without running, so not one of :data:`PRINT_COMMANDS`.
    operands
        The operand words printed alongside it.

    Returns
    -------
    tuple[tuple[str, tuple[str, ...]], ...]
        ``(script, commands_it_runs)`` pairs. The command list is empty for a
        form that is nothing but a comment.
    """
    joined = " ".join((command, *operands))
    return (
        (f'echo "{joined}"', ("echo",)),
        (f"echo '{joined}'", ("echo",)),
        (f"printf '%s\\n' '{joined}'", ("printf",)),
        (f"# {joined}", ()),
        (f"true; # {joined}", ("true",)),
        (f"echo {command}", ("echo",)),
    )
