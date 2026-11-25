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


def test_the_strict_export_is_an_executable_line_in_the_recipe() -> None:
    """Require the export to be a live recipe line, not merely present.

    A commented-out assignment still satisfies a substring search, so a reader
    that only searched the file would accept its removal. This reads the
    target's recipe and ignores comments, which is the same distinction the
    shell makes.
    """
    recipe = _recipe_of(MAKEFILE_PATH.read_text(encoding="utf-8"), TARGET)
    assert recipe, f"the `{TARGET}` target must have a recipe"
    executed = _executable_lines(recipe)
    assert any(EXPORT_VARIABLE in line for line in executed), (
        "the target's recipe must export the strict-mode variable on an "
        f"executable line; the recipe lines are {executed}"
    )


def test_the_strict_export_check_rejects_a_commented_assignment() -> None:
    """Prove the recipe check fires on a commented-out export.

    Without this, a check that happened to pass on the live file would say
    nothing about whether it can detect the removal it exists to catch.
    """
    commented = (
        f"{TARGET}:\n\t# {EXPORT_VARIABLE}\n\tcargo nextest run\n\nother:\n\ttrue\n"
    )
    recipe = _recipe_of(commented, TARGET)
    assert recipe, "the control must yield a recipe"
    assert len(recipe) == 3, f"the recipe must stop at the next rule, got {recipe}"
    executed = _executable_lines(recipe)
    assert not any(EXPORT_VARIABLE in line for line in executed), (
        "a commented-out export must not count as an executed one"
    )
    assert any(EXPORT_VARIABLE in line for line in recipe), (
        "the control must still *contain* the text, or it would not exercise the "
        "comment filter at all"
    )


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
