"""Hold the `test-kani-scope-wrapper` recipe to what makes it meaningful.

The target's strict-mode default and its export are the difference between a
lane that proves the wrapper works and one that skips every scenario on a host
without a per-user manager and still reports green. A substring search over the
whole Makefile accepts a commented-out assignment, so these contracts read the
target's recipe and distinguish the lines a shell would run from the ones it
would not.

Split from ``kani_scope_wrapper_docs_test.py`` to keep both files inside the
repository's 400-line module limit. The document contract lives there; this
module owns the Makefile half.

Run via ``make test-workflow-contracts``.
"""

import re

from workflow_loading import MAKEFILE_PATH

#: The variable the suite reads as strict mode, and the value that enables it.
EXPORT_VARIABLE = "NETSUKE_KANI_SCOPE_WRAPPER_STRICT=$(KANI_SCOPE_WRAPPER_STRICT)"

#: The target these contracts are about.
TARGET = "test-kani-scope-wrapper"


def test_makefile_defaults_strict_mode_and_exports_it() -> None:
    """Require the strict default and the target itself to exist.

    A lane that lost the default would run the suite with no strict mode: every
    scenario would skip on a host without a per-user manager and the lane would
    still report green.
    """
    text = MAKEFILE_PATH.read_text(encoding="utf-8")
    assert re.search(rf"^{TARGET}:", text, re.MULTILINE), (
        f"the Makefile must keep the `{TARGET}` target"
    )
    assert re.search(r"^KANI_SCOPE_WRAPPER_STRICT\s*\?=\s*1$", text, re.MULTILINE), (
        "`KANI_SCOPE_WRAPPER_STRICT` must default to 1"
    )


def test_the_strict_export_reaches_the_test_command() -> None:
    """Require the export and the command to be one shell invocation.

    Three things have to hold and none implies another. The assignment must be
    an executable recipe line rather than a comment; it must be joined to the
    command by a trailing backslash, because Make runs each recipe line in its
    own shell; and it must *export*, which means prefixing the command or using
    an explicit `export`. `VAR=x; cargo ...` satisfies the first two and still
    leaves the variable unexported, so strict mode would stay off while every
    check passed.
    """
    recipe = _recipe_of(MAKEFILE_PATH.read_text(encoding="utf-8"), TARGET)
    assert recipe, f"the `{TARGET}` target must have a recipe"
    statement = _single_statement_with(recipe, EXPORT_VARIABLE)
    lines = _executable_lines(recipe)
    assert statement is not None, (
        "the target's recipe must carry the strict-mode export on an "
        f"executable line continued into the command; the recipe lines are {lines}"
    )
    assert "nextest" in statement or "cargo" in statement, (
        "the export must be joined to the test command by a line continuation, "
        f"or Make runs it in a separate shell; the joined statement was {statement!r}"
    )
    assert _exports_to_command(statement), (
        "the assignment must reach the command's environment: put it before the "
        "command as a prefix, or use an explicit `export`. A `NAME=value; "
        "command` sequence sets a shell variable and exports nothing; got "
        f"{statement!r}"
    )


def test_the_strict_export_check_rejects_a_commented_assignment() -> None:
    """Prove the check fires on a commented-out export."""
    commented = (
        f"{TARGET}:\n\t# {EXPORT_VARIABLE}\n\tcargo nextest run\n\nother:\n\ttrue\n"
    )
    recipe = _recipe_of(commented, TARGET)
    assert recipe, "the control must yield a recipe"
    assert len(recipe) == 3, f"the recipe must stop at the next rule, got {recipe}"
    assert _single_statement_with(recipe, EXPORT_VARIABLE) is None, (
        "a commented-out export must not count as an executed one"
    )
    assert any(EXPORT_VARIABLE in line for line in recipe), (
        "the control must still *contain* the text, or it would not exercise the "
        "comment filter at all"
    )


def test_the_strict_export_check_rejects_a_dropped_continuation() -> None:
    """Prove the check fires when only the backslash is removed.

    This is the defect a presence check cannot see: the text is unchanged and
    still on an executable line, but Make now runs it in a shell of its own.
    """
    broken = f"{TARGET}:\n\t{EXPORT_VARIABLE}\n\tcargo nextest run\n"
    recipe = _recipe_of(broken, TARGET)
    statement = _single_statement_with(recipe, EXPORT_VARIABLE)
    assert statement is not None, "the control must still yield a statement"
    assert "cargo" not in statement, (
        "without the continuation the export must not be joined to the command, "
        f"so the check above can reject it; the joined statement was {statement!r}"
    )
    assert "nextest" not in statement, (
        "without the continuation the export must not be joined to the test "
        f"command; the joined statement was {statement!r}"
    )


def test_the_strict_export_check_rejects_an_unexported_assignment() -> None:
    """Prove the check fires when the assignment merely precedes the command.

    Measured: `VAR=x; env | grep ^VAR=` finds nothing, while `VAR=x env |
    grep ^VAR=` finds it. The control below is the first shape, which the
    continuation and comment checks both accept.
    """
    unexported = f"{TARGET}:\n\t{EXPORT_VARIABLE}; cargo nextest run\n"
    recipe = _recipe_of(unexported, TARGET)
    statement = _single_statement_with(recipe, EXPORT_VARIABLE)
    assert statement is not None, "the control must still yield a statement"
    assert "cargo" in statement, (
        "the control must join the assignment to the command, or it would test "
        f"the continuation check instead; got {statement!r}"
    )
    assert not _exports_to_command(statement), (
        "a `NAME=value; command` sequence exports nothing and the check must "
        f"say so; got {statement!r}"
    )


def test_the_strict_export_check_rejects_an_inline_comment() -> None:
    """Prove an export quoted in a trailing comment does not count.

    The recipe runs the tests with no strict-mode assignment and mentions one
    after a `#`, which a reader that only searched each line would accept.
    """
    export = f"export {EXPORT_VARIABLE}"
    commented = f"{TARGET}:\n\tcargo nextest run # {export}\n"
    recipe = _recipe_of(commented, TARGET)
    statement = _single_statement_with(recipe, EXPORT_VARIABLE)
    assert statement is None, (
        "an export inside a trailing comment must not count as executed; "
        f"got {statement!r}"
    )
    assert any(EXPORT_VARIABLE in line for line in _executable_lines(recipe)), (
        "the control must still *contain* the text on an executable line, or it "
        "would not exercise the comment filter"
    )


def test_the_strict_export_check_rejects_a_comment_between_assignment_and_command() -> (
    None
):
    """Prove a comment line ends the shell invocation.

    Make passes recipe comments to the shell, so a comment line without a
    trailing backslash terminates the command: the assignment above it never
    reaches the command below. Stripping comments before joining would hide
    that boundary.
    """
    separated = (
        f"{TARGET}:\n"
        f"\t{EXPORT_VARIABLE} \\\n"
        "\t# a note between them\n"
        "\tcargo nextest run\n"
    )
    recipe = _recipe_of(separated, TARGET)
    statement = _single_statement_with(recipe, EXPORT_VARIABLE)
    assert statement is not None, "the control must still find the assignment"
    assert "cargo" not in statement, (
        "a comment line must end the invocation, so the command is not in the "
        f"same statement as the assignment; got {statement!r}"
    )


def test_the_strict_export_check_rejects_a_double_ampersand_chain() -> None:
    """Prove the check fires on a chained assignment too.

    `NAME=value && cargo ...` looks like a prefix to a reader and is measured
    not to export: the assignment is its own simple command.
    """
    chained = f"{TARGET}:\n\t{EXPORT_VARIABLE} && cargo nextest run\n"
    recipe = _recipe_of(chained, TARGET)
    statement = _single_statement_with(recipe, EXPORT_VARIABLE)
    assert statement is not None, "the control must still yield a statement"
    assert "cargo" in statement, (
        "the control must join the assignment to the command, or it would test "
        f"the continuation check instead; got {statement!r}"
    )
    assert not _exports_to_command(statement), (
        "a `NAME=value && command` chain exports nothing and the check must "
        f"say so; got {statement!r}"
    )


def _exports_to_command(statement: str) -> bool:
    """Whether the strict-mode variable reaches the command's environment.

    Two forms work: `export NAME=value` on its own, and a prefix assignment
    `NAME=value command`. A bare `NAME=value; command` sets a shell variable
    and exports nothing.

    Returns
    -------
    bool
        True when the variable is exported rather than merely assigned.
    """
    if f"export {EXPORT_VARIABLE}" in statement:
        return True
    if EXPORT_VARIABLE not in statement:
        return False
    # A prefix assignment leaves the command immediately after the value. A
    # command separator of any kind means the assignment ended a command of its
    # own: `NAME=value && cmd` and `NAME=value; cmd` both set a shell variable
    # and export nothing, which is measured behaviour rather than a guess.
    remainder = statement.split(EXPORT_VARIABLE, 1)[1].lstrip()
    if not remainder:
        return False
    return not remainder.startswith((";", "&&", "||", "|", "&"))


def _single_statement_with(recipe: list[str], needle: str) -> str | None:
    r"""Return the shell statement containing ``needle``, continuations joined.

    Make hands each recipe line to its own shell, and a trailing backslash is
    what joins two lines into one invocation. This models that: consecutive
    executable lines are concatenated while the previous one ends in `\\`.

    Returns
    -------
    str | None
        The joined statement containing ``needle``, or ``None`` when no
        executable statement contains it.
    """
    statements: list[str] = []
    current = ""
    for line in recipe:
        # Continuations are joined across *all* recipe lines, comments
        # included, because that is how the shell sees them: a comment line
        # without a trailing backslash ends the invocation, so an assignment
        # above it never reaches the command below. Stripping comments first
        # would silently join across that boundary.
        if not line.startswith(("\t", " ")):
            continue
        stripped = line.rstrip()
        continued = stripped.endswith("\\")
        current += stripped.removesuffix("\\")
        if not continued:
            statements.append(_strip_inline_comment(current))
            current = ""
    if current:
        statements.append(_strip_inline_comment(current))
    for statement in statements:
        if needle in statement and _assigns_to_command(statement, needle):
            return statement
    return None


def _recipe_of(text: str, target: str) -> list[str]:
    """Return the recipe lines of one Makefile target, stopping at the next rule.

    A line at column zero ends the recipe, whether it is another rule or a
    comment; Make reads it the same way.

    Returns
    -------
    list[str]
        The lines following ``target``'s header, up to the first line at column
        zero. Empty when the target is absent.
    """
    lines = text.splitlines()
    header = f"{target}:"
    starts = [index for index, line in enumerate(lines) if line.startswith(header)]
    if not starts:
        return []
    body: list[str] = []
    for line in lines[starts[0] + 1 :]:
        if line and not line.startswith(("\t", " ")):
            break
        body.append(line)
    return body


def _strip_inline_comment(line: str) -> str:
    """Return ``line`` with any trailing shell comment removed.

    A `#` at the start of a word begins a comment; a `#` inside a word does
    not, so a token such as `abc#def` survives. This matters here because a
    comment can otherwise quote an export the command never runs.

    Returns
    -------
    str
        The executable part of the line.
    """
    marker = line.find(" #")
    if marker < 0 and line.lstrip().startswith("#"):
        return ""
    return line if marker < 0 else line[:marker]


def _assigns_to_command(statement: str, needle: str) -> bool:
    """Whether ``needle`` appears before any comment in ``statement``.

    Returns
    -------
    bool
        True when the needle is part of the executed text.
    """
    return needle in _strip_inline_comment(statement)


def _executable_lines(recipe: list[str]) -> list[str]:
    """Return the recipe lines a shell would run.

    Returns
    -------
    list[str]
        The indented lines whose first non-blank character is not `#`.
    """
    return [
        line
        for line in recipe
        if line.startswith(("\t", " ")) and not line.lstrip().startswith("#")
    ]
