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

    Two things have to hold, and neither implies the other. The assignment must
    be an executable recipe line rather than a comment, and it must be joined to
    the command by a trailing backslash: Make runs each recipe line in its own
    shell, so an assignment whose continuation was dropped exports nothing to
    the `cargo nextest` line that follows. A check for the assignment alone
    accepts exactly that defect.
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
    for line in _executable_lines(recipe):
        stripped = line.rstrip()
        continued = stripped.endswith("\\")
        current += stripped.removesuffix("\\")
        if not continued:
            statements.append(current)
            current = ""
    if current:
        statements.append(current)
    return next((statement for statement in statements if needle in statement), None)


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
