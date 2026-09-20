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
    match = re.search(
        rf"\b{re.escape(command)}\b(?P<operands>[^\n]*)",
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

    Parameters
    ----------
    segment
        One command segment, as :func:`command_segments` returns them.
    command
        The command whose output is captured, without its path.

    Returns
    -------
    set[str]
        The assigned names whose value contains the command, which is the name
        its output is captured in.
    """
    return {
        match.group("name")
        for match in ASSIGNMENT.finditer(segment)
        if re.search(rf"\b{re.escape(command)}\b", match.group("value"))
    }


def names_a_component(operand: str, name: str, *, prefix: bool = False) -> bool:
    """Whether ``operand`` is a path naming ``name``.

    The name is matched as a whole path component, so an operand naming a file
    that merely *ends* in the report's name is not the report. Shell syntax
    around a component — `"${staged}/x"`, `$staged/x`, or the unquoted form —
    is stripped first, so the three spellings of one directory compare equal.

    With ``prefix`` set, a component the name *starts* is matched too, which is
    how a directory is named as the head of a longer path.

    Parameters
    ----------
    operand
        One word taken from a command segment.
    name
        The path component to look for.
    prefix
        Whether a component merely beginning with ``name`` counts.

    Returns
    -------
    bool
        Whether one of the operand's components is ``name``.
    """
    return any(
        _matches_component(component, name, prefix=prefix)
        for component in operand.strip("\"'").split("/")
    )


def _matches_component(component: str, name: str, *, prefix: bool) -> bool:
    """Whether one path component names ``name``.

    Returns
    -------
    bool
        Whether ``component`` equals ``name``, or begins with it when
        ``prefix`` is set.
    """
    cleaned = component.strip("\"'${}")
    if prefix:
        return cleaned.startswith(name)
    return cleaned == name
