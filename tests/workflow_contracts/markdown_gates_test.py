"""Contract tests for the Markdown formatting and lint gates.

``make fmt`` and ``make check-fmt`` call ``mdtablefix`` directly: ``--git
--include-untracked`` takes the file list from Git, ``--in-place`` rewrites
it, and ``--check`` reports drift read-only. Both recipes share one selection
and one rule set, declared once in the Makefile, so the formatter and its gate
cannot disagree about which files or which rules they cover.

Markdown linting in CI runs through the upstream ``markdownlint-cli2`` action
rather than an installation of our own, pinned to a commit and reading the
same ``.markdownlint-cli2.jsonc`` as ``make markdownlint``. That configuration
is JSONC, so the reader below accepts what the linter accepts: the two comment
forms and a trailing comma. A stricter reader would fail the gate over a file
the linter is perfectly happy with.

Run via ``make test-workflow-contracts``.
"""

import json
import re

from markdown_gates import read_jsonc
from workflow_loading import (
    MAKEFILE_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    require_mapping,
    step_runs,
)

#: The linter configuration both `make markdownlint` and the CI action read.
MARKDOWNLINT_CONFIG = REPO_ROOT / ".markdownlint-cli2.jsonc"

#: The JSONC shapes the linter accepts and `json` alone refuses, each with the
#: value the reader must recover from it. Measured against markdownlint-cli2
#: 0.22.1 (markdownlint 0.40.0), whose parser runs `jsonc-parser` with
#: `allowTrailingComma`; every case below is accepted there and rejected by a
#: reader that handles whole-line comments only.
JSONC_CASES = (
    ('{\n  // note\n  "ignores": [".uv-cache/**"]\n}', [".uv-cache/**"]),
    ('{\n  "ignores": [".uv-cache/**"] // note\n}', [".uv-cache/**"]),
    ('/* lead */\n{\n  "ignores": [".uv-cache/**"]\n}', [".uv-cache/**"]),
    ('{\n  /*\n   block\n  */\n  "ignores": [".uv-cache/**"]\n}', [".uv-cache/**"]),
    ('{\n  "ignores": [".uv-cache/**"],\n}', [".uv-cache/**"]),
    ('{\n  "ignores": [".uv-cache/**",]\n}', [".uv-cache/**"]),
    ('{\n  "ignores": [".uv-cache/**", /* note */],\n}', [".uv-cache/**"]),
    ('{\n  "ignores": [\n    // note\n    ".uv-cache/**"\n  ]\n}', [".uv-cache/**"]),
    # A comment opener inside a string is content, not syntax: the globs and
    # URLs this file holds carry both, and cutting at them would corrupt them.
    ('{"globs": ["**/*.md"], "ignores": ["//g"]}', ["//g"]),
    ('{"u": "https://e.com//x", "ignores": ["/*g*/"]}', ["/*g*/"]),
    ('{"a": "q\\"//b", "ignores": ["x//y"]}', ["x//y"]),
)


def test_the_jsonc_reader_accepts_what_the_linter_accepts() -> None:
    """Each JSONC shape the linter parses reads back with its globs intact."""
    for text, expected in JSONC_CASES:
        declared = require_mapping(read_jsonc(text), "the linter configuration").get(
            "ignores"
        )
        assert declared == expected, (
            f"reading JSONC {text!r} must yield {expected!r}, got {declared!r}"
        )


def test_the_jsonc_reader_still_rejects_text_the_linter_rejects() -> None:
    """Comment stripping must not turn malformed JSONC into a passing read.

    This is the half that keeps the reader honest: a stripping rule loose
    enough to accept anything would let the contract pass over a file the
    linter refuses.

    Raises
    ------
    AssertionError
        If a malformed text reads as JSON instead of being rejected.
    """
    for text in ('{ "config": }', '{ "config": {},', "// nothing but a comment"):
        try:
            read_jsonc(text)
        except json.JSONDecodeError:
            continue
        message = f"malformed JSONC {text!r} must not read as JSON"
        raise AssertionError(message)


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


#: Shell-side Markdown selection the recipes must not carry. mdtablefix owns
#: the file list through `--git`, and mdtablefix 0.6.0 defines the empty
#: selection as a success that exits `0`, prints nothing, and reads no stdin,
#: so no shell guard for an empty file list is needed or permitted.
SHELL_SELECTION_FRAGMENTS = (
    "check-markdown-format",
    "$(MD_FILES_FIND)",
    "find ",
    "xargs",
    "sh -c",
    '"$$@"',
    '"$$#"',
    "--in-place",
)


def test_makefile_check_fmt_runs_mdtablefix_check_over_git_selection() -> None:
    """`make check-fmt` asks mdtablefix directly whether the tree drifts.

    Selection is delegated wholesale to `$(MDTABLEFIX_SELECT)`: the recipe
    must carry no `find`, `xargs`, positional-parameter forwarding, or
    empty-input guard of its own, because `mdtablefix --git` selects the
    files and treats selecting nothing as a success.
    """
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
    for retired in SHELL_SELECTION_FRAGMENTS:
        assert retired not in recipe, (
            f"check-fmt must delegate selection to $(MDTABLEFIX_SELECT) rather "
            f"than the retired shell-side {retired!r}: {recipe!r}"
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

#: The commit the `v24.2.0` tag names. The pin must be this object and not the
#: annotated tag object of the same release, which `ci.yml` carried until this
#: contract asserted the value: both are forty hex characters, so only the
#: value tells them apart. Resolve it again with
#: `gh api repos/DavidAnson/markdownlint-cli2-action/git/tags/v24.2.0`.
#:
#: Asserted by name where the developer guide's "Workflow pins and Dependabot"
#: rule otherwise prescribes shape only. That rule's cost is a lockstep edit
#: on every Dependabot bump, and it does not arise here: the version comment
#: beside the pin names no value, and Dependabot's subject and commit message
#: both carry the incoming tag, so the version and this constant move in one
#: commit. A bump that moves the pin without moving this constant fails here,
#: and that is what the assertion is for.
RELEASE_COMMIT = "21c1be1b93ad9ed58fa840aacc3f279cde2a72ff"


def test_build_job_lints_markdown_through_the_upstream_action() -> None:
    """The Linux merge gate lints Markdown with the SHA-pinned upstream action.

    The Makefile keeps `make markdownlint` for local use, but CI must not
    install markdownlint-cli2 itself: the action step is the gate, it must be
    pinned to a commit rather than a tag, and it must lint the same globs as
    the Makefile so the two agree on which files are covered.

    The pin is asserted by value and not only by shape, so that reverting to
    the annotated tag object of the same release fails here. Nothing else
    catches that: both objects are forty hex characters.
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
    assert ref == RELEASE_COMMIT, (
        f"the markdownlint action must be pinned to the commit v24.2.0 names, "
        f"{RELEASE_COMMIT!r}; {ref!r} is a different object. An annotated tag "
        f"object is not the commit it points at"
    )
    with_ = require_mapping(lint_steps[0].get("with"), "Lint Markdown inputs")
    assert with_.get("globs") == "**/*.md", (
        f"the markdownlint action must lint every Markdown file, got {with_!r}"
    )
    assert "make markdownlint" not in step_runs(steps), (
        "build-test must not also install and run markdownlint-cli2 itself"
    )


#: Every ignore glob the estate's canonical `.markdownlint-cli2.jsonc` lists,
#: copied verbatim from `platform-standards/canon/lint/markdown/` in the
#: concordat repository. Written out here rather than fetched, because a
#: contract that read the canon over the network would be a gate on somebody
#: else's availability; the cost is that a canon change needs this list
#: changed with it, which is the point at which somebody decides to adopt it.
#:
#: The baseline is a floor rather than the whole list. A repository may ignore
#: more, and this one does. `.uv-cache/**` is the entry this repository had
#: narrowed rather than lost: it carried `**/.uv-cache/**` alone, which is the
#: *wider* of the two, since `**/` matches zero directories as readily as
#: several and so covers a cache at the repository root. The canon form was
#: simply absent. Restoring it is conformance rather than coverage: measured on
#: markdownlint-cli2 0.22.1, dropping `.uv-cache/**` again excludes the same
#: files, because `**/.uv-cache/**` already covers them.
BASELINE_IGNORES: tuple[str, ...] = (
    "**/.venv/**",
    ".vtcode/**",
    "**/node_modules/**",
    "**/target/**",
    ".terraform/**",
    ".uv-cache/**",
    "memories/**",
    "CRUSH.md",
)


def test_the_linter_configuration_keeps_every_baseline_ignore() -> None:
    """PD-005: each baseline glob is present verbatim, not merely in spirit.

    A missing entry is the failure this catches, and the check is of presence
    rather than of effect: the baseline entry may be redundant with an extra
    glob the repository already lists, as `.uv-cache/**` is here, and it is
    still required. A canon entry that silently stops being listed is drift a
    reader comparing the two files by eye would have to notice.

    Extra ignores are allowed and this repository has several. Only the
    absence of a baseline entry is an offence.
    """
    text = MARKDOWNLINT_CONFIG.read_text(encoding="utf-8")
    declared = require_mapping(read_jsonc(text), "the linter configuration").get(
        "ignores", []
    )
    # A mapping would match the baseline against its keys, and a string would
    # match it as a substring, so either shape passes this test while the
    # linter rejects the file. Only a list of globs is an answer.
    assert isinstance(declared, list), (
        f"ignores must be a JSON array of glob strings, got {declared!r}"
    )
    assert all(isinstance(glob, str) for glob in declared), (
        f"ignores must be a JSON array of glob strings, got {declared!r}"
    )
    missing = [glob for glob in BASELINE_IGNORES if glob not in declared]
    assert not missing, (
        f"{MARKDOWNLINT_CONFIG.name} must list every baseline ignore glob "
        f"verbatim; it is missing {missing}. A glob that reads as equivalent "
        f"is not the same glob"
    )
