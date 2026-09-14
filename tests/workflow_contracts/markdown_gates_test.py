"""Contract tests for the Markdown formatting and lint gates.

``make fmt`` and ``make check-fmt`` call ``mdtablefix`` directly: ``--git
--include-untracked`` takes the file list from Git, ``--in-place`` rewrites
it, and ``--check`` reports drift read-only. Both recipes share one selection
and one rule set, declared once in the Makefile, so the formatter and its gate
cannot disagree about which files or which rules they cover.

Markdown linting in CI runs through the upstream ``markdownlint-cli2`` action
rather than an installation of our own, pinned to a commit and reading the
same ``.markdownlint-cli2.jsonc`` as ``make markdownlint``.

Run via ``make test-workflow-contracts``.
"""

import re

from workflow_loading import (
    MAKEFILE_PATH,
    job_steps,
    load_workflow,
    require_mapping,
    step_runs,
)


def _makefile_recipe(target: str) -> tuple[list[str], str]:
    """Return a Makefile target's recipe lines and their joined text."""
    makefile_lines = MAKEFILE_PATH.read_text(encoding="utf-8").splitlines()
    target_index = next(
        (
            index
            for index, line in enumerate(makefile_lines)
            if line.startswith(f"{target}:")
        ),
        None,
    )
    assert target_index is not None, f"the Makefile must define a {target} target"

    top_level_target = re.compile(r"^[A-Za-z0-9_.%/-]+:")
    recipe_lines = []
    for line in makefile_lines[target_index + 1 :]:
        if top_level_target.match(line):
            break
        if line.startswith("\t"):
            recipe_lines.append(line)
    return recipe_lines, "\n".join(recipe_lines)


def _makefile_variable(name: str) -> str:
    """Return the value a simple Makefile variable assignment declares."""
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    match = re.search(rf"^{re.escape(name)}\s*\??=\s*(.*)$", makefile, re.MULTILINE)
    assert match is not None, f"the Makefile must declare {name}"
    return match.group(1).strip()


#: The mdtablefix selection every Markdown formatting recipe must use. `--git`
#: takes the file list from Git's index and `--include-untracked` adds the
#: untracked files Git does not ignore, so a new document is formatted and
#: checked before it is staged.
MDTABLEFIX_SELECTION = "--git --include-untracked"

#: The formatting rules `make fmt` applies and `make check-fmt` verifies.
#: Held equal to the Makefile so the two cannot drift.
MDTABLEFIX_RULES = "--wrap --renumber --breaks --ellipsis --fences"


def test_makefile_declares_the_mdtablefix_selection_and_rules() -> None:
    """The selection and rule flags are declared once and shared by both."""
    select = _makefile_variable("MDTABLEFIX_SELECT")
    rules = _makefile_variable("MDTABLEFIX_RULES")

    assert select == MDTABLEFIX_SELECTION, (
        f"MDTABLEFIX_SELECT must be {MDTABLEFIX_SELECTION!r}, got {select!r}"
    )
    assert rules == MDTABLEFIX_RULES, (
        f"MDTABLEFIX_RULES must be {MDTABLEFIX_RULES!r}, got {rules!r}"
    )


def test_makefile_check_fmt_runs_mdtablefix_check_over_git_selection() -> None:
    """`make check-fmt` asks mdtablefix directly whether the tree drifts."""
    recipe_lines, recipe = _makefile_recipe("check-fmt")
    mdtablefix_lines = [line for line in recipe_lines if "$(MDTABLEFIX)" in line]

    assert len(mdtablefix_lines) == 1, (
        f"check-fmt must invoke $(MDTABLEFIX) exactly once, got {recipe!r}"
    )
    invocation = mdtablefix_lines[0].split()
    assert invocation[:2] == ["$(MDTABLEFIX)", "--check"], (
        f"check-fmt must run mdtablefix in read-only --check mode, got {invocation!r}"
    )
    assert "$(MDTABLEFIX_SELECT)" in invocation, (
        "check-fmt must select files with $(MDTABLEFIX_SELECT)"
    )
    assert "$(MDTABLEFIX_RULES)" in invocation, (
        "check-fmt must verify the shared $(MDTABLEFIX_RULES)"
    )
    for retired in ("--in-place", "check-markdown-format", "$(MD_FILES_FIND)", "xargs"):
        assert retired not in recipe, (
            f"check-fmt must not use the retired {retired!r} path: {recipe!r}"
        )


def test_makefile_fmt_runs_mdtablefix_then_markdownlint_fix() -> None:
    """`make fmt` rewrites the Git selection in place, then applies lint fixes."""
    recipe_lines, recipe = _makefile_recipe("fmt")
    mdtablefix_index = next(
        (index for index, line in enumerate(recipe_lines) if "$(MDTABLEFIX)" in line),
        None,
    )
    mdlint_index = next(
        (index for index, line in enumerate(recipe_lines) if "$(MDLINT)" in line),
        None,
    )

    assert mdtablefix_index is not None, f"fmt must invoke $(MDTABLEFIX): {recipe!r}"
    assert mdlint_index is not None, f"fmt must invoke $(MDLINT): {recipe!r}"
    assert mdtablefix_index < mdlint_index, (
        "fmt must run mdtablefix before markdownlint-cli2 --fix, because "
        "mdtablefix owns table padding and paragraph wrapping"
    )
    invocation = recipe_lines[mdtablefix_index].split()
    assert invocation[:2] == ["$(MDTABLEFIX)", "--in-place"], (
        f"fmt must rewrite Markdown with mdtablefix --in-place, got {invocation!r}"
    )
    assert "$(MDTABLEFIX_SELECT)" in invocation, (
        "fmt must select files with $(MDTABLEFIX_SELECT)"
    )
    assert "$(MDTABLEFIX_RULES)" in invocation, (
        "fmt must apply the shared $(MDTABLEFIX_RULES)"
    )
    mdlint_line = recipe_lines[mdlint_index]
    assert "--fix" in mdlint_line.split(), (
        f"fmt must run markdownlint-cli2 with --fix, got {mdlint_line!r}"
    )
    assert "mdformat-all" not in recipe, (
        "fmt must call the formatters directly rather than through mdformat-all"
    )


#: The upstream markdownlint-cli2 action. Its release carries the linter's
#: whole dependency graph, so nothing is resolved from the registry at run
#: time, and Dependabot manages the pin alongside the other actions.
MARKDOWNLINT_ACTION = "DavidAnson/markdownlint-cli2-action@"


def test_build_job_lints_markdown_through_the_upstream_action() -> None:
    """The Linux merge gate lints Markdown with the SHA-pinned upstream action.

    The Makefile keeps `make markdownlint` for local use, but CI must not
    install markdownlint-cli2 itself: the action step is the gate, it must be
    pinned to a commit rather than a tag, and it must lint the same globs as
    the Makefile so the two agree on which files are covered.
    """
    steps = job_steps(load_workflow(), "build-test")
    lint_steps = [
        step
        for step in steps
        if str(step.get("uses", "")).startswith(MARKDOWNLINT_ACTION)
    ]

    assert len(lint_steps) == 1, (
        f"build-test must lint Markdown through {MARKDOWNLINT_ACTION} exactly once"
    )
    uses = str(lint_steps[0]["uses"])
    ref = uses.removeprefix(MARKDOWNLINT_ACTION)
    assert re.fullmatch(r"[0-9a-f]{40}", ref), (
        f"the markdownlint action must be pinned to a commit SHA, got {ref!r}"
    )
    with_ = require_mapping(lint_steps[0].get("with"), "Lint Markdown inputs")
    assert with_.get("globs") == "**/*.md", (
        f"the markdownlint action must lint every Markdown file, got {with_!r}"
    )
    assert "make markdownlint" not in step_runs(steps), (
        "build-test must not also install and run markdownlint-cli2 itself"
    )
