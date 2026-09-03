"""Contract tests for the CI workflow's lint and test-shell prerequisites.

Two invariants in ``.github/workflows/ci.yml`` are load-bearing but easy to
lose to an innocuous edit, and both fail in ways that are slow and confusing
to diagnose from a red run:

* The hermetic dev-fast sandbox probes ``awk`` through a capability-backed
  directory handle, which deliberately cannot follow a symlink that escapes
  its containing directory. Ubuntu ships ``awk`` as exactly such a symlink,
  so the job installs ``gawk`` and copies it to a regular executable on the
  job ``PATH``. Dropping any part of that dance turns into a sandbox probe
  failure far from its cause.
* ``make lint`` must run, and the Makefile's Clippy flags must stay
  workspace-wide. Narrowing them silently stops linting ``test_support``
  and the non-default targets.

The ``setup-rust`` input contract and the single-declaration ``NEXTEST_VERSION``
pin live here too, because both govern how the lint and test gates are
provisioned. The Windows job's own contracts live in
``ci_windows_job_test.py`` and the coverage wiring in
``ci_coverage_wiring_test.py``; the shared parsing helpers in
``workflow_loading.py``.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

import pytest
from workflow_loading import (
    CI_WORKFLOW_PATH,
    MAKEFILE_PATH,
    SETUP_RUST_JOBS,
    _WorkflowLoader,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    step_runs,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

TEST_SHELL_STEP = "Install test shell dependencies"

#: Clippy must stay workspace-wide with warnings denied; a narrower flag set
#: silently drops `test_support` and the non-default targets from the gate.
EXPECTED_CLIPPY_FLAGS = "--workspace --all-targets --all-features -- -D warnings"

#: The whole awk staging dance the sandbox probe depends on. Asserting the
#: fragments as one block reports every part a partial edit dropped, rather
#: than only the first.
AWK_STAGING_FRAGMENTS = (
    'test_shell_bin="${RUNNER_TEMP}/netsuke-test-bin"',
    'install --mode=0755 "$(command -v gawk)"',
    '"${test_shell_bin}/awk"',
)


def _tool_input(step: dict[str, object]) -> object:
    """Return a step's ``with.tool`` input, or ``""`` when it declares none."""
    match step.get("with"):
        case {"tool": tool}:
            return tool
        case _:
            return ""


def test_workflow_loader_keeps_github_actions_trigger_words_as_strings() -> None:
    """Keep GitHub Actions trigger words distinct from YAML boolean literals."""
    loader = _WorkflowLoader("on: on\nyes: yes\noff: off\nno: no\n")
    try:
        document = loader.get_single_data()
    finally:
        loader.dispose()

    for word in ("on", "yes", "off", "no"):
        assert document[word] == word, (
            f"the GitHub Actions {word!r} trigger word must stay a string "
            "rather than becoming a YAML 1.1 boolean"
        )
        assert isinstance(document[word], str), (
            f"the GitHub Actions {word!r} trigger word must remain a string "
            "for workflow parsing"
        )
    assert "on" in document, "the GitHub Actions on: trigger key must remain a string"
    assert True not in document, (
        "the GitHub Actions on: trigger key must not become True"
    )


@pytest.mark.parametrize(
    ("literal", "expected"),
    [
        pytest.param("true", True, id="lowercase-true"),
        pytest.param("True", True, id="titlecase-true"),
        pytest.param("TRUE", True, id="uppercase-true"),
        pytest.param("false", False, id="lowercase-false"),
        pytest.param("False", False, id="titlecase-false"),
        pytest.param("FALSE", False, id="uppercase-false"),
    ],
)
def test_workflow_loader_boolean_literals(literal: str, *, expected: bool) -> None:
    """Resolve YAML 1.2 boolean literals in every permitted case variant."""
    loader = _WorkflowLoader(f"value: {literal}\n")
    try:
        document = loader.get_single_data()
    finally:
        loader.dispose()

    assert document == {"value": expected}, (
        f"the YAML 1.2 literal {literal!r} must resolve to {expected!r}"
    )


@pytest.mark.parametrize(
    ("document", "expected_fragment"),
    [
        pytest.param("", "got NoneType", id="empty-document"),
        pytest.param("build-test\n", "got str", id="scalar-document"),
        pytest.param("- build-test\n", "got list", id="list-document"),
        pytest.param(
            "true: build-test\n",
            "string-keyed",
            id="non-string-mapping-key",
        ),
    ],
)
def test_load_workflow_rejects_invalid_document_roots(
    tmp_path: Path,
    document: str,
    expected_fragment: str,
) -> None:
    """Reject malformed documents before workflow-specific contracts run."""
    workflow_path = tmp_path / "workflow.yml"
    workflow_path.write_text(document, encoding="utf-8")

    # ``pytest.fail`` raises this private exception so tests can assert the
    # loader's user-facing validation message without treating it as an error.
    with pytest.raises(pytest.fail.Exception, match=re.escape(expected_fragment)):
        load_workflow(workflow_path)


@pytest.fixture
def test_shell_script() -> str:
    """Return the run script of the test-shell dependency step."""
    step = named_step(job_steps(load_workflow(), "build-test"), TEST_SHELL_STEP)
    match step.get("run"):
        case str() as run:
            return run
        case _:
            pytest.fail(f"{TEST_SHELL_STEP} must declare a run script")


def test_test_shell_step_installs_gawk(test_shell_script: str) -> None:
    """The step installs gawk, the implementation awk is copied from."""
    assert re.search(r"apt-get install\b.*\bgawk\b", test_shell_script), (
        f"{TEST_SHELL_STEP} must apt-get install gawk, got:\n{test_shell_script}"
    )


def test_test_shell_step_copies_gawk_to_a_regular_awk_executable(
    test_shell_script: str,
) -> None:
    """Gawk is copied — not linked — to ${RUNNER_TEMP}/netsuke-test-bin/awk.

    The sandbox probe cannot follow a symlink out of its directory handle, so
    the destination must be a regular executable file.
    """
    missing = [
        fragment
        for fragment in AWK_STAGING_FRAGMENTS
        if fragment not in test_shell_script
    ]
    assert not missing, (
        f"{TEST_SHELL_STEP} must copy $(command -v gawk) into "
        f"${{RUNNER_TEMP}}/netsuke-test-bin as a regular awk executable; "
        f"missing {missing!r}, got:\n{test_shell_script}"
    )


def test_test_shell_step_exports_the_directory_to_github_path(
    test_shell_script: str,
) -> None:
    """Later steps see the staged awk because the directory joins PATH."""
    assert 'echo "${test_shell_bin}" >> "${GITHUB_PATH}"' in test_shell_script, (
        f"{TEST_SHELL_STEP} must append the staging directory to "
        f"GITHUB_PATH, got:\n{test_shell_script}"
    )


def test_test_shell_step_verifies_the_staged_awk(test_shell_script: str) -> None:
    """The step proves the staged awk resolves and runs before CI proceeds."""
    lines = [line.strip() for line in test_shell_script.splitlines()]
    assert "command -v awk" in lines, (
        f"{TEST_SHELL_STEP} must run `command -v awk`, got:\n{test_shell_script}"
    )
    assert "awk --version" in lines, (
        f"{TEST_SHELL_STEP} must run `awk --version`, got:\n{test_shell_script}"
    )


def test_workflow_runs_make_lint() -> None:
    """CI runs the lint gate through the trusted Makefile binary."""
    runs = step_runs(job_steps(load_workflow(), "build-test"))
    expected = '/usr/bin/make ACTIONLINT="$GITHUB_WORKSPACE/actionlint" lint'
    assert expected in runs, (
        f"the build-test job must run `{expected}`, got run steps: {runs!r}"
    )


def test_workflow_runs_make_typecheck() -> None:
    """CI runs the typecheck gate through the Makefile, not an ad hoc command."""
    runs = step_runs(job_steps(load_workflow(), "build-test"))
    assert "make typecheck" in runs, (
        f"the build-test job must run `make typecheck`, got run steps: {runs!r}"
    )


def test_makefile_clippy_flags_stay_workspace_wide() -> None:
    """CLIPPY_FLAGS covers the whole workspace with warnings denied."""
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    match = re.search(r"^CLIPPY_FLAGS \?= (.+)$", makefile, re.MULTILINE)
    assert match is not None, "the Makefile must define CLIPPY_FLAGS"
    assert match.group(1).strip() == EXPECTED_CLIPPY_FLAGS, (
        f"CLIPPY_FLAGS must be {EXPECTED_CLIPPY_FLAGS!r} so Clippy covers "
        f"every workspace crate, target, and feature with warnings denied; "
        f"got {match.group(1).strip()!r}"
    )


def test_makefile_check_fmt_runs_markdown_format_checker() -> None:
    """Protect NUL-safe Markdown batching and its portable empty-input guard."""
    makefile_lines = MAKEFILE_PATH.read_text(encoding="utf-8").splitlines()
    target_index = next(
        (
            index
            for index, line in enumerate(makefile_lines)
            if line.startswith("check-fmt:")
        ),
        None,
    )
    assert target_index is not None, "the Makefile must define a check-fmt target"

    top_level_target = re.compile(r"^[A-Za-z0-9_.%/-]+:")
    recipe_lines = []
    for line in makefile_lines[target_index + 1 :]:
        if top_level_target.match(line):
            break
        if line.startswith("\t"):
            recipe_lines.append(line)
    recipe = "\n".join(recipe_lines)

    required_fragments = {
        "$(MD_FILES_FIND)": "discover Markdown files with $(MD_FILES_FIND)",
        "scripts/check-markdown-format.sh": "invoke the Markdown format checker",
        "xargs -0": "batch Markdown paths with NUL delimiters",
        "sh -c": "run the portable empty-input guard",
        'if [ "$$#" -gt 0 ]': "skip the Markdown checker for empty input",
        'scripts/check-markdown-format.sh "$$@"': (
            "validate every discovered Markdown path"
        ),
    }
    missing_fragments = [
        description
        for fragment, description in required_fragments.items()
        if fragment not in recipe
    ]
    assert not missing_fragments, "check-fmt must " + "; ".join(missing_fragments)

    xargs_arguments = [
        argument
        for line in recipe_lines
        if "xargs" in line
        for argument in line.split()
    ]
    assert not any(
        argument in {"-r", "--no-run-if-empty"}
        or (
            argument.startswith("-")
            and not argument.startswith("--")
            and "r" in argument[1:]
        )
        for argument in xargs_arguments
    ), (
        "check-fmt must use the shell positional-parameter guard instead of "
        f"GNU-only xargs -r, found xargs arguments: {xargs_arguments!r}"
    )


def test_nextest_version_declared_once_at_workflow_scope() -> None:
    r"""NEXTEST_VERSION is declared once, at workflow scope.

    AGENTS.md documents a local-install recipe that extracts the pin with:

        sed -n "s/.*NEXTEST_VERSION: '\(.*\)'.*/\1/p" \
            .github/workflows/ci.yml

    A job-scoped duplicate would make that command emit two newline-separated
    values, which `cargo install --version "$NEXTEST_VERSION"` rejects. The pin
    therefore lives in the workflow-level `env:` block — the only declaration
    in the file — and both jobs read it via `${{ env.NEXTEST_VERSION }}`.
    """
    text = CI_WORKFLOW_PATH.read_text(encoding="utf-8")
    declarations = re.findall(r"^\s*NEXTEST_VERSION:\s*'([^']+)'", text, re.MULTILINE)
    assert declarations == ["0.9.133"], (
        "NEXTEST_VERSION must be declared exactly once at workflow scope "
        f"with the pinned value, got {declarations!r}"
    )

    workflow = load_workflow()
    env = require_mapping(workflow.get("env"), "the workflow-level env")
    assert env.get("NEXTEST_VERSION") == "0.9.133", (
        "NEXTEST_VERSION must be pinned at workflow scope, "
        f"got {env.get('NEXTEST_VERSION')!r}"
    )

    for workflow_path, job_name in SETUP_RUST_JOBS:
        installs = [
            tool
            for step in job_steps(load_workflow(workflow_path), job_name)
            if "nextest" in str(tool := _tool_input(step))
        ]
        assert installs == ["nextest@${{ env.NEXTEST_VERSION }}"], (
            f"{job_name} must install nextest via the workflow-scoped "
            f"${{{{ env.NEXTEST_VERSION }}}}, got {installs!r}"
        )


def test_setup_rust_does_not_pass_unsupported_components_input() -> None:
    """No setup-rust invocation passes the unsupported `components` input.

    The shared `setup-rust` action installs `rustfmt` and `clippy` internally
    through `actions-rust-lang/setup-rust-toolchain`; its declared inputs do not
    include `components`, so passing one emits an "Unexpected input(s)
    'components'" warning on every run. The contract is that every `Setup Rust`
    step uses the shared action and passes only supported inputs, so `check-fmt`
    and `lint-clippy` still find the components the action installs.
    """
    for workflow_path, job_name in SETUP_RUST_JOBS:
        steps = job_steps(load_workflow(workflow_path), job_name)
        setup_steps = [
            step for step in steps if "setup-rust" in str(step.get("uses", ""))
        ]
        assert setup_steps, f"{job_name} must use the shared setup-rust action"
        for step in setup_steps:
            with_ = require_mapping(
                step.get("with"), f"{job_name} Setup Rust's with block"
            )
            assert "components" not in with_, (
                f"{job_name} Setup Rust must not pass the unsupported "
                f"'components' input, got {sorted(with_.keys())!r}"
            )


def test_build_job_runs_markdown_formatter_checker_tests() -> None:
    """The Linux merge gate exercises the Markdown checker process boundary."""
    runs = step_runs(job_steps(load_workflow(), "build-test"))

    assert runs.count("make test-markdown-format") == 1, (
        "build-test must run the Markdown checker test suite exactly once"
    )
