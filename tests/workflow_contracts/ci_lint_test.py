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

These tests parse the workflow with PyYAML and pin that contract, so drift
fails on the pull request rather than in a later run.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
from pathlib import Path

import pytest
import yaml

REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "ci.yml"
MAKEFILE_PATH = REPO_ROOT / "Makefile"

TEST_SHELL_STEP = "Install test shell dependencies"

#: Clippy must stay workspace-wide with warnings denied; a narrower flag set
#: silently drops `test_support` and the non-default targets from the gate.
EXPECTED_CLIPPY_FLAGS = "--workspace --all-targets --all-features -- -D warnings"


class _WorkflowLoader(yaml.SafeLoader):
    """Loader that resolves booleans the YAML 1.2 way.

    PyYAML implements YAML 1.1, where ``on``, ``yes``, and ``off`` are boolean
    words. That silently turns GitHub Actions' ``on:`` trigger key into
    ``True``. Mapping ``True`` back to ``"on"`` after the fact would conflate it
    with a literal ``yes:`` or ``true:`` key, so the resolver is narrowed to
    YAML 1.2's ``true``/``false`` instead and ``on`` simply stays a string.
    """


# Drop the inherited YAML 1.1 bool resolver, then reinstate the 1.2 word set.
_WorkflowLoader.yaml_implicit_resolvers = {
    initial: [
        (tag, regexp)
        for tag, regexp in resolvers
        if tag != "tag:yaml.org,2002:bool"
    ]
    for initial, resolvers in yaml.SafeLoader.yaml_implicit_resolvers.items()
}
_WorkflowLoader.add_implicit_resolver(
    "tag:yaml.org,2002:bool",
    re.compile(r"^(?:true|True|TRUE|false|False|FALSE)$"),
    list("tTfF"),
)


def _load() -> dict[str, object]:
    """Parse the workflow file, rejecting anything but a mapping root.

    ``yaml.safe_load`` happily returns ``None`` for an empty document, or a
    scalar or list for a malformed one. Without a runtime check the annotation
    is a claim rather than a guarantee, and the failure surfaces later as an
    opaque ``AttributeError`` far from the real cause.
    """
    # `yaml.load` is safe here: `_WorkflowLoader` derives from `SafeLoader`, so
    # it constructs no arbitrary Python objects.
    match yaml.load(
        WORKFLOW_PATH.read_text(encoding="utf-8"), Loader=_WorkflowLoader
    ):
        case dict() as workflow:
            pass
        case other:
            pytest.fail(
                "the workflow must parse to a mapping, "
                f"got {type(other).__name__}"
            )
    non_string_keys = sorted(repr(key) for key in workflow if not isinstance(key, str))
    if non_string_keys:
        pytest.fail(
            f"the workflow mapping must be string-keyed, got {non_string_keys}"
        )
    return workflow


def _steps(workflow: dict[str, object]) -> list[dict[str, object]]:
    """Return the build-test job's steps."""
    match workflow.get("jobs"):
        case dict() as jobs:
            pass
        case _:
            pytest.fail("the workflow must declare a jobs mapping")
    match jobs.get("build-test"):
        case dict() as job:
            pass
        case _:
            pytest.fail("the workflow must declare a build-test job")
    match job.get("steps"):
        case list() as steps:
            return steps
        case _:
            pytest.fail("jobs.build-test.steps must be a list")


def _windows_job(workflow: dict[str, object]) -> dict[str, object]:
    """Return the build-test-windows job."""
    match workflow.get("jobs"):
        case dict() as jobs:
            pass
        case _:
            pytest.fail("the workflow must declare a jobs mapping")
    match jobs.get("build-test-windows"):
        case dict() as job:
            return job
        case _:
            pytest.fail(
                "the workflow must declare a build-test-windows job"
            )


def _windows_steps(workflow: dict[str, object]) -> list[dict[str, object]]:
    """Return the build-test-windows job's steps."""
    match _windows_job(workflow).get("steps"):
        case list() as steps:
            return steps
        case _:
            pytest.fail("jobs.build-test-windows.steps must be a list")


def _windows_step(name: str) -> dict[str, object]:
    """Return the uniquely named step from the build-test-windows job."""
    matches = [
        step for step in _windows_steps(_load()) if step.get("name") == name
    ]
    assert len(matches) == 1, (
        f"expected exactly one build-test-windows step named {name!r}, "
        f"found {len(matches)}"
    )
    return matches[0]


def _step(name: str) -> dict[str, object]:
    """Return the uniquely named step from the build-test job."""
    matches = [step for step in _steps(_load()) if step.get("name") == name]
    assert len(matches) == 1, (
        f"expected exactly one step named {name!r}, found {len(matches)}"
    )
    return matches[0]


def _test_shell_script() -> str:
    """Return the run script of the test-shell dependency step."""
    match _step(TEST_SHELL_STEP).get("run"):
        case str() as run:
            return run
        case _:
            pytest.fail(f"{TEST_SHELL_STEP} must declare a run script")


def test_test_shell_step_installs_gawk() -> None:
    """The step installs gawk, the implementation awk is copied from."""
    script = _test_shell_script()
    assert re.search(r"apt-get install\b.*\bgawk\b", script), (
        f"{TEST_SHELL_STEP} must apt-get install gawk, got:\n{script}"
    )


def test_test_shell_step_copies_gawk_to_a_regular_awk_executable() -> None:
    """gawk is copied — not linked — to ${RUNNER_TEMP}/netsuke-test-bin/awk.

    The sandbox probe cannot follow a symlink out of its directory handle, so
    the destination must be a regular executable file.
    """
    script = _test_shell_script()
    assert 'install --mode=0755 "$(command -v gawk)"' in script, (
        f"{TEST_SHELL_STEP} must copy $(command -v gawk) as a regular "
        f"executable, got:\n{script}"
    )
    assert '"${test_shell_bin}/awk"' in script, (
        f"{TEST_SHELL_STEP} must install the copy as awk, got:\n{script}"
    )
    assert 'test_shell_bin="${RUNNER_TEMP}/netsuke-test-bin"' in script, (
        f"{TEST_SHELL_STEP} must stage the copy in "
        f"${{RUNNER_TEMP}}/netsuke-test-bin, got:\n{script}"
    )


def test_test_shell_step_exports_the_directory_to_github_path() -> None:
    """Later steps see the staged awk because the directory joins PATH."""
    script = _test_shell_script()
    assert 'echo "${test_shell_bin}" >> "${GITHUB_PATH}"' in script, (
        f"{TEST_SHELL_STEP} must append the staging directory to "
        f"GITHUB_PATH, got:\n{script}"
    )


def test_test_shell_step_verifies_the_staged_awk() -> None:
    """The step proves the staged awk resolves and runs before CI proceeds."""
    script = _test_shell_script()
    lines = [line.strip() for line in script.splitlines()]
    assert "command -v awk" in lines, (
        f"{TEST_SHELL_STEP} must run `command -v awk`, got:\n{script}"
    )
    assert "awk --version" in lines, (
        f"{TEST_SHELL_STEP} must run `awk --version`, got:\n{script}"
    )


def test_workflow_runs_make_lint() -> None:
    """CI runs the lint gate through the Makefile, not an ad hoc command."""
    runs = [step.get("run") for step in _steps(_load())]
    assert "make lint" in runs, (
        f"the build-test job must run `make lint`, got run steps: {runs!r}"
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


def test_nextest_version_declared_once_at_workflow_scope() -> None:
    """NEXTEST_VERSION is declared once, at workflow scope.

    AGENTS.md documents a local-install recipe that extracts the pin with:

        sed -n "s/.*NEXTEST_VERSION: '\\(.*\\)'.*/\\1/p" \
            .github/workflows/ci.yml

    A job-scoped duplicate would make that command emit two newline-separated
    values, which `cargo install --version "$NEXTEST_VERSION"` rejects. The pin
    therefore lives in the workflow-level `env:` block — the only declaration
    in the file — and both jobs read it via `${{ env.NEXTEST_VERSION }}`.
    """
    text = WORKFLOW_PATH.read_text(encoding="utf-8")
    declarations = re.findall(r"^\s*NEXTEST_VERSION:\s*'([^']+)'", text, re.MULTILINE)
    assert declarations == ["0.9.133"], (
        "NEXTEST_VERSION must be declared exactly once at workflow scope "
        f"with the pinned value, got {declarations!r}"
    )

    workflow = _load()
    match workflow.get("env"):
        case dict() as env:
            pass
        case _:
            pytest.fail(
                "the workflow must declare a workflow-level env mapping"
            )
    assert env.get("NEXTEST_VERSION") == "0.9.133", (
        "NEXTEST_VERSION must be pinned at workflow scope, "
        f"got {env.get('NEXTEST_VERSION')!r}"
    )

    for job_name in ("build-test", "build-test-windows"):
        match workflow.get("jobs"):
            case dict() as jobs:
                pass
            case _:
                pytest.fail("the workflow must declare a jobs mapping")
        match jobs.get(job_name):
            case dict() as job:
                pass
            case _:
                pytest.fail(f"the workflow must declare a {job_name} job")
        assert "NEXTEST_VERSION" not in job.get("env", {}), (
            f"{job_name} must not redeclare NEXTEST_VERSION at job scope"
        )
        installs = [
            step.get("with", {}).get("tool")
            for step in job.get("steps", [])
            if "nextest" in str(step.get("with", {}).get("tool", ""))
        ]
        assert installs == ["nextest@${{ env.NEXTEST_VERSION }}"], (
            f"{job_name} must install nextest via the workflow-scoped "
            f"${{{{ env.NEXTEST_VERSION }}}}, got {installs!r}"
        )


def test_windows_job_runs_on_windows_latest() -> None:
    """The Windows job must actually run on a Windows runner."""
    job = _windows_job(_load())
    assert job.get("runs-on") == "windows-latest", (
        "build-test-windows must run on windows-latest so the "
        f"#[cfg(windows)] tree is compiled, got {job.get('runs-on')!r}"
    )


def test_windows_job_uses_git_bash_for_recipes() -> None:
    """The job runs recipes under Git Bash, not cmd.exe.

    The Makefile uses POSIX shell constructs throughout, and GNU Make's
    default recipe shell on Windows is cmd.exe, so the job must default every
    run step to bash.
    """
    job = _windows_job(_load())
    match job.get("defaults"):
        case dict() as defaults:
            pass
        case _:
            pytest.fail(
                "build-test-windows must declare a defaults mapping"
            )
    match defaults.get("run"):
        case dict() as run:
            pass
        case _:
            pytest.fail(
                "build-test-windows must declare a defaults.run mapping"
            )
    assert run.get("shell") == "bash", (
        "build-test-windows must run recipes under Git Bash "
        f"(defaults.run.shell: bash), got {run.get('shell')!r}"
    )


def test_windows_setup_rust_keeps_warnings_and_polonius() -> None:
    """The Windows toolchain setup preserves -D warnings and -Zpolonius=next.

    The `#[cfg(windows)]` tree must be compiled under `-D warnings` to surface
    findings, and the tree requires the Polonius analysis, so the shared
    setup-rust action must receive both flags through its `rustflags` input.
    """
    step = _windows_step("Setup Rust")
    assert "setup-rust" in step.get("uses", ""), (
        f"Setup Rust must use the shared setup-rust action, got {step.get('uses')!r}"
    )
    match step.get("with"):
        case dict() as with_:
            pass
        case _:
            pytest.fail("Setup Rust must declare a with mapping")
    assert with_.get("toolchain") == "${{ env.NETSUKE_RUST_TOOLCHAIN }}", (
        "Setup Rust must use the pinned NETSUKE_RUST_TOOLCHAIN, "
        f"got {with_.get('toolchain')!r}"
    )
    assert with_.get("rustflags") == "-D warnings -Zpolonius=next", (
        "Setup Rust must pass -D warnings -Zpolonius=next through rustflags "
        f"so the #[cfg(windows)] tree compiles under warnings-as-errors, "
        f"got {with_.get('rustflags')!r}"
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
    workflow = _load()
    for job_name in ("build-test", "build-test-windows"):
        match workflow.get("jobs"):
            case dict() as jobs:
                pass
            case _:
                pytest.fail("the workflow must declare a jobs mapping")
        match jobs.get(job_name):
            case dict() as job:
                pass
            case _:
                pytest.fail(f"the workflow must declare a {job_name} job")
        setup_steps = [
            step
            for step in job.get("steps", [])
            if "setup-rust" in str(step.get("uses", ""))
        ]
        assert setup_steps, f"{job_name} must use the shared setup-rust action"
        for step in setup_steps:
            match step.get("with"):
                case dict() as with_:
                    pass
                case _:
                    pytest.fail(
                        f"{job_name} Setup Rust must declare a with mapping"
                    )
            assert "components" not in with_, (
                f"{job_name} Setup Rust must not pass the unsupported "
                f"'components' input, got {sorted(with_.keys())!r}"
            )


def test_windows_job_runs_check_fmt_lint_and_test() -> None:
    """The Windows job runs check-fmt, lint, and test as merge gates.

    Every quality gate must run through the Makefile with `SHELL=bash` so the
    POSIX-shell recipes execute under Git Bash on the Windows runner.
    """
    runs = [step.get("run") for step in _windows_steps(_load())]
    expected = [
        "make SHELL=bash check-fmt",
        "make SHELL=bash lint-clippy",
        "make SHELL=bash lint-whitaker",
        "make SHELL=bash test",
    ]
    for command in expected:
        assert command in runs, (
            f"build-test-windows must run {command!r}, got run steps: {runs!r}"
        )


def test_windows_job_does_not_duplicate_doc_and_audit_gates() -> None:
    """The Windows job excludes platform-independent doc and audit gates.

    `make spelling`, `make markdownlint`, `make nixie`, coverage generation,
    the CodeScene gate, and `make test-workflow-contracts` are already covered
    on Linux; duplicating them on Windows buys nothing.
    """
    runs = [step.get("run") for step in _windows_steps(_load())]
    excluded = [
        "make spelling",
        "make markdownlint",
        "make nixie",
        "make test-workflow-contracts",
    ]
    for command in excluded:
        assert command not in runs, (
            f"build-test-windows must not run the platform-independent "
            f"{command!r}, got run steps: {runs!r}"
        )


def test_windows_job_is_a_blocking_merge_gate() -> None:
    """No step in the Windows job is allowed to fail silently.

    A `continue-on-error: true` on the job or any step would let a Windows
    lint or test failure pass the merge, defeating the gate.
    """
    job = _windows_job(_load())
    assert job.get("continue-on-error") is not True, (
        "build-test-windows must not set continue-on-error on the job"
    )
    for step in _windows_steps(_load()):
        assert step.get("continue-on-error") is not True, (
            f"build-test-windows step {step.get('name')!r} must not set "
            "continue-on-error"
        )


def test_coverage_report_is_produced_before_codescene_check() -> None:
    """The CodeScene gate consumes the report the coverage step produces.

    `generate-coverage` writes the report to `output-path` (lcov.info) and the
    `upload-codescene-coverage` check step reads that exact path in lcov
    format. If the report path, format, or step ordering drifts, CodeScene
    reports "No valid coverage report found in the build pipeline". This pins
    the wiring: the coverage step runs after `make test` and the CodeScene
    check runs after the coverage step, both with `format: lcov`.
    """
    workflow = _load()
    steps = _steps(workflow)
    names = [step.get("name") for step in steps]

    coverage_index = names.index("Test and Measure Coverage")
    codescene_index = names.index("Check coverage against CodeScene gates")
    test_index = names.index("Test")
    assert test_index < coverage_index, (
        "coverage must be measured after the test run so the report reflects "
        "the tested tree"
    )
    assert coverage_index < codescene_index, (
        "the CodeScene check must run after the coverage step so the report "
        "exists in the build pipeline"
    )

    coverage_step = steps[coverage_index]
    match coverage_step.get("with"):
        case dict() as with_:
            pass
        case _:
            pytest.fail(
                "Test and Measure Coverage must declare a with mapping"
            )
    assert with_.get("output-path") == "lcov.info", (
        "coverage must be written to lcov.info, "
        f"got {with_.get('output-path')!r}"
    )
    assert with_.get("format") == "lcov", (
        "coverage must be measured in lcov format, "
        f"got {with_.get('format')!r}"
    )

    codescene_step = steps[codescene_index]
    match codescene_step.get("with"):
        case dict() as with_:
            pass
        case _:
            pytest.fail(
                "Check coverage against CodeScene gates must declare a with "
                "mapping"
            )
    assert with_.get("format") == "lcov", (
        "the CodeScene check must consume lcov format, "
        f"got {with_.get('format')!r}"
    )
    assert with_.get("path") == "lcov.info", (
        "the CodeScene check must read the report generated at lcov.info, "
        f"got {with_.get('path')!r}"
    )
    assert with_.get("mode") == "check", (
        "the CodeScene check must run in check mode, "
        f"got {with_.get('mode')!r}"
    )
