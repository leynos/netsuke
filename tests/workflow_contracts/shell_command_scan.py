"""Reading a shell script as the commands it runs.

A shell script is not a list of lines. A `run: |` block happens to put one
command per line, but the same shell accepts a one-liner joining several with
`&&`, and a predicate stated over lines would read such a line as a single
command: a match could begin in the command that copies a file and finish in
the command that names a directory, certifying work that was never done.

The functions here answer what such a predicate asks of shell text — which
simple commands a script runs, which words one of them is handed, and which
names an assignment captures a command's output in — and each answers it about
one command rather than about one line. They read text rather than parsed
YAML, so they are general to any `run:` block: nothing here knows what the
script is for, and the caller supplies the command names and the paths it
cares about.

Splitting on the separators is a deliberate over-approximation. A `;` or a `|`
inside a quoted string is not a separator, and cutting there can therefore
divide a command in two — which is the safe direction. A rule asserting that
two things appear in one command is only ever made *harder* to satisfy by
cutting more finely, so a script misjudged by the split is reported rather
than passed, and its author moves the two things into the same command.

Separated from ``codescene_report_validation_invariants`` so neither module
outgrows the 400-line limit the Python lint gate enforces.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

#: Matches one shell separator, splitting a line into the simple commands it
#: runs. `&&` and `||` are matched before the single-character class so each is
#: consumed whole rather than as its first character followed by another.
COMMAND_SEPARATOR: typ.Final[re.Pattern[str]] = re.compile(r"&&|\|\||[;|]")

#: One shell variable assignment, as `name=value`. A quoted value is read to
#: its closing quote — command substitution inside one contains spaces — and an
#: unquoted value to the next space, which is where the shell ends it too.
ASSIGNMENT: typ.Final[re.Pattern[str]] = re.compile(
    r"(?:^|\s)(?P<name>[A-Za-z_]\w*)=(?P<value>\"[^\"]*\"|'[^']*'|\S*)"
)

#: What may stand between the start of a segment and the command it runs:
#: leading whitespace, `VAR=value` assignments, and the path the executable was
#: named through. A command is read from here rather than searched for in the
#: segment, because a *mention* of a command is not an invocation of it:
#: `echo "cp a b"` names `cp` and runs it not at all, and a rule reading that
#: mention would certify work the script never did.
COMMAND_PREFIX: typ.Final[str] = (
    r"\s*"
    r"(?:[A-Za-z_]\w*=(?:\"[^\"]*\"|'[^']*'|\S*)\s+)*"
    r"(?:\S*/)?"
)


def command_segments(script: str) -> list[str]:
    """Return the script's commands, one segment per simple shell command.

    A shell script is not a list of lines. A `run: |` block happens to put one
    command per line, but the same shell accepts a one-liner joining several
    with `&&`, and a predicate stated over lines would read such a line as a
    single command: a match could begin in the command that copies a file and
    finish in the command that names the staged directory, reporting a report
    that was never put into it.

    Splitting on the separators is a deliberate over-approximation. A `;` or a
    `|` inside a quoted string is not a separator, and cutting there can
    therefore divide a command in two — which is the safe direction. A rule
    asserting that two things appear in one command is only ever made *harder*
    to satisfy by cutting more finely, so a script misjudged by the split is
    reported rather than passed, and its author moves the report path and the
    directory into the same command.

    Returns
    -------
    list[str]
        The segments, in order, including any that are blank.
    """
    return [
        segment
        for line in script.splitlines()
        for segment in COMMAND_SEPARATOR.split(line)
    ]


def command_operands(segment: str, command: str) -> list[str]:
    """Return the words a command segment hands to ``command``, in order.

    Order is load-bearing for a copying command, whose last operand is where
    the file goes: read as a set of words, `cp "${staged}/lcov.info" lcov.info`
    and `cp lcov.info "${staged}/lcov.info"` are the same command, and a
    contract that cannot tell them apart certifies a file taken *out* of a
    directory as one put into it.

    The name is read where a command *runs*, not wherever it appears. A
    segment that prints, quotes, or comments the words of a command mentions it
    and runs it not at all, and a rule reading the mention would certify work
    the script never did — `echo "cp src dest"` would satisfy a rule asking
    that the file be copied. So the name is matched after the segment's leading
    whitespace, any `VAR=value` assignments before it, and the path it was
    named through, and not after another word: `echo cp`, and `/bin/echo cp`,
    are a different command being handed this one as an argument. A form this
    does not recognise — `sudo cp`, `xargs cp`, `else cp` — is reported rather
    than accepted, which is the safe direction: the spelling these rules are
    written for is the ordinary one, and the author moves the command into the
    position a command is read from.

    Flags are dropped, so `mkdir --parents -- x` yields `x` rather than the
    options that precede it. A word that names a directory and begins with a
    hyphen is not recognised, which costs nothing where these are used: a
    staged scratch directory is not named that way.

    Parameters
    ----------
    segment
        One command segment, as :func:`command_segments` returns them.
    command
        The command whose operands are wanted, without its path.

    Returns
    -------
    list[str]
        The operand words in the order written, empty when the segment does not
        run ``command``.
    """
    match = re.match(
        COMMAND_PREFIX + rf"{re.escape(command)}\b(?P<operands>[^\n]*)",
        segment,
    )
    if match is None:
        return []
    return [
        word for word in match.group("operands").split() if not word.startswith("-")
    ]


def assigned_from(segment: str, command: str) -> set[str]:
    """Return the names a segment assigns ``command``'s output to.

    Only the assignment *holding* the command counts. `staged=staging
    scratch="$(mktemp -d)"` assigns two names and creates one directory: read
    as a set of names, `staged` would be recorded as holding a directory —
    and a script whose staged directory is a bare literal nothing created
    would then pass a creation check on the strength of the neighbour
    assignment that made something else.

    The command has to be *run* for its output to be captured, so it is read
    inside a command substitution. A value that merely spells the command —
    `staged="mktemp -d"` — assigns a string and creates nothing, and reading
    that spelling as a capture would record a directory the script never made.

    Parameters
    ----------
    segment
        One command segment, as :func:`command_segments` returns them.
    command
        The command whose output is captured, without its path.

    Returns
    -------
    set[str]
        The assigned names whose value captures the command's output, which is
        the name its output is captured in.
    """
    inside_substitution = re.compile(rf"\$\([^)]*\b{re.escape(command)}\b")
    return {
        match.group("name")
        for match in ASSIGNMENT.finditer(segment)
        if inside_substitution.search(match.group("value"))
    }


def names_a_component(operand: str, name: str, *, prefix: bool = False) -> bool:
    """Whether ``operand`` is a path naming ``name``.

    The name is matched as a whole path component, so an operand naming a file
    that merely *ends* in the report's name is not the report. Shell syntax
    around a component — `"${staged}/x"`, `$staged/x`, or the unquoted form —
    is stripped first, so the three spellings of one directory compare equal.

    With ``prefix`` set, the name has to be the operand's *leading* component,
    which is how a path names something inside a directory: `"${staged}/x"` is
    under the named directory and `staged` alone *is* it. Equality is what
    makes the difference, not a string prefix — `"${staged}-old/x"` is a
    sibling directory whose name merely begins with this one's, and the file
    copied there lands nowhere the caller is about to read. A name in a later
    component is a different location entirely.

    Parameters
    ----------
    operand
        One word taken from a command segment.
    name
        The path component to look for.
    prefix
        Whether the name must be the operand's leading component.

    Returns
    -------
    bool
        Whether one of the operand's components is ``name``, or — when
        ``prefix`` is set — whether the first one is.
    """
    components = [
        component.strip("\"'${}") for component in operand.strip("\"'").split("/")
    ]
    if prefix:
        return bool(components) and components[0] == name
    return name in components
