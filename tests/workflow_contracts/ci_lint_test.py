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

import pytest
from workflow_loading import (
    CI_WORKFLOW_PATH,
    MAKEFILE_PATH,
    job_steps,
    load_workflow,
    named_step,
    require_mapping,
    step_runs,
)

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

#: Jobs whose `setup-rust` invocations must stay on the shared action's
#: supported input set.
SETUP_RUST_JOBS = ("build-test", "build-test-windows")


def _tool_input(step: dict[str, object]) -> object:
    """Return a step's ``with.tool`` input, or ``""`` when it declares none."""
    match step.get("with"):
        case {"tool": tool}:
            return tool
        case _:
            return ""


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
    """CI runs the lint gate through the Makefile, not an ad hoc command."""
    runs = step_runs(job_steps(load_workflow(), "build-test"))
    assert "make lint" in runs, (
        f"the build-test job must run `make lint`, got run steps: {runs!r}"
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
    """Protect Markdown formatting from being silently removed from make check-fmt."""
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
    expected_pipeline = (
        "@$(MD_FILES_FIND) | xargs -0 -r scripts/check-markdown-format.sh"
    )

    required_fragments = {
        "$(MD_FILES_FIND)": "discover Markdown files with $(MD_FILES_FIND)",
        "scripts/check-markdown-format.sh": "invoke the Markdown format checker",
        "xargs -0 -r": "batch Markdown paths with NUL delimiters and skip empty input",
    }
    missing_fragments = [
        description
        for fragment, description in required_fragments.items()
        if fragment not in recipe
    ]
    assert not missing_fragments, "check-fmt must " + "; ".join(missing_fragments)
    assert recipe.count(expected_pipeline) == 1, (
        "check-fmt must contain exactly one Markdown format checker pipeline"
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

    for job_name in SETUP_RUST_JOBS:
        installs = [
            tool
            for step in job_steps(workflow, job_name)
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
    workflow = load_workflow()
    for job_name in SETUP_RUST_JOBS:
        steps = job_steps(workflow, job_name)
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


def test_mdtablefix_installers_require_the_pinned_version() -> None:
    """Both formatter installers replace stale executables and verify the pin."""
    expected_guard = 'expected_mdtablefix_version="mdtablefix ${MDTABLEFIX_VERSION}"'
    expected_match = (
        '[[ "${installed_mdtablefix_version}" != "${expected_mdtablefix_version}" ]]'
    )
    workflow = load_workflow()
    for job_name in SETUP_RUST_JOBS:
        step = named_step(
            job_steps(workflow, job_name),
            "Install mdtablefix",
        )
        match step.get("run"):
            case str() as run:
                assert expected_guard in run, (
                    f"{job_name} must pin the expected version"
                )
                assert "mdtablefix --version" in run, (
                    f"{job_name} must inspect the installed version"
                )
                assert "tr -d '\\r'" in run, (
                    f"{job_name} must normalise Windows version output"
                )
                assert expected_match in run, (
                    f"{job_name} must replace a missing or mismatched formatter"
                )
            case _:
                pytest.fail(f"{job_name} must configure mdtablefix")


def test_build_job_runs_markdown_formatter_checker_tests() -> None:
    """The Linux merge gate exercises the Markdown checker process boundary."""
    runs = step_runs(job_steps(load_workflow(), "build-test"))

    assert runs.count("make test-markdown-format") == 1, (
        "build-test must run the Markdown checker test suite exactly once"
    )
