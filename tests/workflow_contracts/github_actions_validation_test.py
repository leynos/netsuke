"""Contract tests for the GitHub Actions validation toolchain.

The workflow linters are installed only on the Linux CI job and run through
the Makefile's ``lint`` target. These checks keep the YAML policy, trusted
tool provisioning, and Makefile wiring in step without testing either
third-party linter's own behaviour.

Run via ``make test-workflow-contracts``.
"""

# This test's contract is the controlled Make invocation, not a linter's
# implementation.
# ruff: ignore[suspicious-subprocess-import] - the boundary is under test.
import subprocess
import typing as typ

from actionlint_installer_contract import (
    ACTIONLINT_CHECKSUM_COMMAND,
    ACTIONLINT_INSTALL_COMMAND,
    ACTIONLINT_SCRIPT_CONTRACTS,
    shell_variable,
)
from cmd_mox import CmdMox
from hypothesis import given, settings
from hypothesis import strategies as st
from workflow_loading import (
    CI_WORKFLOW_PATH,
    REPO_ROOT,
    job_steps,
    load_workflow,
    named_step,
)

if typ.TYPE_CHECKING:
    from pathlib import Path


pytest_plugins = ("cmd_mox.pytest_plugin",)

MAKEFILE_PATH = REPO_ROOT / "Makefile"
YAMLLINT_POLICY_PATH = REPO_ROOT / ".yamllint.yml"
WORKFLOW_DIR = REPO_ROOT / ".github" / "workflows"


def _makefile_recipe(target: str) -> list[str]:
    """Return the recipe lines owned by a Makefile target."""
    lines = _read(MAKEFILE_PATH).splitlines()
    target_line = f"{target}:"
    start = next(
        (index for index, line in enumerate(lines) if line.startswith(target_line)),
        None,
    )
    assert start is not None, f"Makefile must define the `{target}` target"

    recipe: list[str] = []
    for line in lines[start + 1 :]:
        if line.startswith("\t"):
            recipe.append(line.removeprefix("\t"))
        elif line and not line.startswith("#"):
            break
    return recipe


def _step_position(steps: list[dict[str, object]], name: str) -> int:
    """Return a required CI step's position for ordering assertions."""
    return steps.index(named_step(steps, name))


def _namespace_cache_paths(steps: list[dict[str, object]]) -> str:
    """Return the Namespace cache volume's mounted paths for the Linux job."""
    cache_step = named_step(steps, "Set up Namespace cache volume")
    inputs = cache_step.get("with")
    assert isinstance(inputs, dict), "the Namespace cache step must declare inputs"
    return str(inputs.get("path", ""))


def _assert_yamllint_ci_contract(steps: list[dict[str, object]]) -> None:
    """Assert that Linux CI provisions the pinned yamllint installation."""
    setup_uv = named_step(steps, "Setup uv")
    install_yamllint = named_step(steps, "Install yamllint")
    cached_paths = _namespace_cache_paths(steps).splitlines()

    assert setup_uv.get("uses") == (
        "astral-sh/setup-uv@11f9893b081a58869d3b5fccaea48c9e9e46f990"
    ), "the Linux CI job must provision uv before installing yamllint"
    setup_uv_inputs = setup_uv.get("with")
    assert isinstance(setup_uv_inputs, dict), "Setup uv must declare inputs"
    assert setup_uv_inputs.get("enable-cache") == "false", (
        "setup-uv must not add a second owner beside the Namespace cache volume"
    )
    for uv_path in (".uv-bin", ".uv-cache", ".uv-tools"):
        assert uv_path in cached_paths, (
            f"the Namespace cache volume must own the uv directory {uv_path}"
        )
    assert install_yamllint.get("run") == (
        'uv tool install "yamllint==${YAMLLINT_VERSION}"\n'
        'echo "${UV_TOOL_BIN_DIR}" >> "$GITHUB_PATH"\n'
    ), "the Linux CI job must install and expose the pinned yamllint binary"


ACTIONLINT_REUSE_GUARD = (
    "if [[ -x ./actionlint ]] \\\n"
    '  && [[ "$(./actionlint --version | head --lines=1)" == '
    f'"{shell_variable("ACTIONLINT_VERSION")}" ]]; then'
)


def _assert_actionlint_ci_contract(steps: list[dict[str, object]]) -> None:
    """Assert that Linux CI provisions actionlint and invokes trusted Make."""
    download_actionlint = named_step(steps, "Download actionlint")
    lint = named_step(steps, "Lint")
    download_script = download_actionlint.get("run")
    cached_paths = _namespace_cache_paths(steps).splitlines()

    assert "actionlint" in cached_paths, (
        "the Namespace cache volume must own the actionlint binary"
    )
    assert isinstance(download_script, str), (
        "the actionlint cache-miss step must define its verified installer script"
    )
    assert ACTIONLINT_REUSE_GUARD in download_script, (
        "the cached actionlint must be reused only when it reports the pinned version"
    )
    assert download_script.index(ACTIONLINT_REUSE_GUARD) < download_script.index(
        ACTIONLINT_INSTALL_COMMAND
    ), "the cached-version guard must precede the installer invocation"
    for expected, message in ACTIONLINT_SCRIPT_CONTRACTS:
        assert expected in download_script, message
    assert download_script.index(ACTIONLINT_CHECKSUM_COMMAND) < download_script.index(
        ACTIONLINT_INSTALL_COMMAND
    ), "the actionlint archive checksum must be verified before running the installer"
    assert lint.get("run") == (
        '/usr/bin/make ACTIONLINT="$GITHUB_WORKSPACE/actionlint" lint'
    ), "the Linux CI job must use trusted `/usr/bin/make` with the cached actionlint"


def _assert_workflow_linter_provisioning_order(
    steps: list[dict[str, object]],
) -> None:
    """Assert that Linux CI provisions both workflow linters before linting."""
    assert all(
        _step_position(steps, earlier) < _step_position(steps, later)
        for earlier, later in [
            ("Set up Namespace cache volume", "Setup uv"),
            ("Setup uv", "Install yamllint"),
            ("Install yamllint", "Download actionlint"),
            ("Download actionlint", "Lint"),
        ]
    ), "the Linux CI job must provision both linters before the trusted lint step"


def _mocked_command(cmd_mox: CmdMox, name: str) -> str:
    """Return the active CmdMox shim for a Makefile command variable."""
    shim_dir = cmd_mox.environment.shim_dir
    assert shim_dir is not None, "CmdMox must create its command shim directory"
    return str(shim_dir / name)


def _run_github_actions_lint(cmd_mox: CmdMox) -> subprocess.CompletedProcess[str]:
    """Run the Makefile target through CmdMox's controlled linter shims."""
    # The command and its arguments are fixed test values; no untrusted input
    # reaches the child process.
    # ruff: ignore[subprocess-without-shell-equals-true] - shell is False.
    return subprocess.run(
        [  # ruff: ignore[start-process-with-partial-path] - CmdMox controls the test command path.
            "make",
            f"YAMLLINT={_mocked_command(cmd_mox, 'yamllint')}",
            f"ACTIONLINT={_mocked_command(cmd_mox, 'actionlint')}",
            "github-actions-lint",
        ],
        check=False,
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
    )


def _read(path: Path) -> str:
    """Read a repository contract file as UTF-8 text."""
    return path.read_text(encoding="utf-8")


def test_makefile_runs_both_github_actions_linters() -> None:
    """The lint target delegates workflow validation to both linters."""
    makefile = _read(MAKEFILE_PATH)
    recipe = _makefile_recipe("github-actions-lint")

    assert (
        "lint: lint-clippy lint-whitaker lint-python github-actions-lint" in makefile
    ), "the `lint` target must delegate workflow validation to `github-actions-lint`"
    assert "github-actions-lint: ## Validate GitHub Actions workflows" in makefile, (
        "the Makefile must document the `github-actions-lint` target"
    )
    assert recipe == [
        "$(YAMLLINT) --config-file .yamllint.yml .github/workflows",
        "$(ACTIONLINT)",
    ], (
        "the `github-actions-lint` recipe must run yamllint with the "
        "repository policy before actionlint"
    )


def test_yamllint_policy_supports_github_actions_workflows() -> None:
    """The YAML policy accepts GitHub's trigger keys and workflow line lengths."""
    policy = _read(YAMLLINT_POLICY_PATH)
    policy_contracts = {
        "extends: default": "the YAML policy must retain yamllint's default rule set",
        "present: true": (
            "the YAML policy must require each workflow to declare a document start"
        ),
        "max: 120": "the YAML policy must cap workflow lines at 120 columns",
        "allowed-values: ['true', 'false']": (
            "the YAML policy must accept quoted GitHub Actions truthy values"
        ),
        "check-keys: false": (
            "the YAML policy must accept GitHub's unquoted `on` trigger key"
        ),
    }

    for expected, message in policy_contracts.items():
        assert expected in policy, message


def test_ci_installs_and_invokes_pinned_workflow_linters() -> None:
    """Linux CI provisions the pinned tools before the trusted lint invocation."""
    steps = job_steps(load_workflow(CI_WORKFLOW_PATH), "build-test")
    _assert_yamllint_ci_contract(steps)
    _assert_actionlint_ci_contract(steps)
    _assert_workflow_linter_provisioning_order(steps)


def test_every_workflow_starts_a_yaml_document() -> None:
    """Every checked workflow declares the document start required by the policy."""
    workflow_paths = sorted({*WORKFLOW_DIR.glob("*.yml"), *WORKFLOW_DIR.glob("*.yaml")})

    assert workflow_paths, "the repository must contain GitHub Actions workflows"
    for workflow_path in workflow_paths:
        assert _read(workflow_path).startswith("---\n"), (
            f"{workflow_path.relative_to(REPO_ROOT)} must begin with a YAML "
            "document start"
        )


def test_github_actions_lint_invokes_linters_in_order(cmd_mox: CmdMox) -> None:
    """The Makefile invokes the configured linters in policy-first order."""
    cmd_mox.mock("yamllint").with_args(
        "--config-file", ".yamllint.yml", ".github/workflows"
    ).returns(exit_code=0).in_order()
    cmd_mox.mock("actionlint").with_args().returns(exit_code=0).in_order()

    result = _run_github_actions_lint(cmd_mox)

    assert result.returncode == 0, (
        "the mocked workflow-lint commands must satisfy the Makefile target; "
        f"stderr was: {result.stderr}"
    )


def test_github_actions_lint_propagates_linter_failure(cmd_mox: CmdMox) -> None:
    """The Makefile stops when yamllint reports a validation failure."""
    cmd_mox.mock("yamllint").with_args(
        "--config-file", ".yamllint.yml", ".github/workflows"
    ).returns(exit_code=23)
    actionlint = cmd_mox.spy("actionlint").returns(exit_code=0)

    result = _run_github_actions_lint(cmd_mox)

    assert result.returncode == 2, (
        "the `github-actions-lint` target must propagate a yamllint failure; "
        f"stderr was: {result.stderr}"
    )
    actionlint.assert_not_called()


def test_github_actions_lint_propagates_actionlint_failure(cmd_mox: CmdMox) -> None:
    """The Makefile fails after actionlint rejects a valid YAML workflow."""
    cmd_mox.mock("yamllint").with_args(
        "--config-file", ".yamllint.yml", ".github/workflows"
    ).returns(exit_code=0).in_order()
    cmd_mox.mock("actionlint").with_args().returns(exit_code=31).in_order()

    result = _run_github_actions_lint(cmd_mox)

    assert result.returncode == 2, (
        "the `github-actions-lint` target must propagate an actionlint failure; "
        f"stderr was: {result.stderr}"
    )


@settings(max_examples=25, deadline=None, derandomize=True)
@given(
    yamllint_exit=st.integers(min_value=0, max_value=255),
    actionlint_exit=st.integers(min_value=0, max_value=255),
)
def test_github_actions_lint_preserves_linter_exit_contract(
    yamllint_exit: int,
    actionlint_exit: int,
) -> None:
    """The Makefile preserves linter ordering and every non-zero exit status."""
    with CmdMox() as cmd_mox:
        yamllint = (
            cmd_mox
            .spy("yamllint")
            .with_args("--config-file", ".yamllint.yml", ".github/workflows")
            .returns(exit_code=yamllint_exit)
            .in_order()
        )
        actionlint = cmd_mox.spy("actionlint").returns(exit_code=actionlint_exit)
        if yamllint_exit == 0:
            actionlint.with_args().in_order()

        cmd_mox.replay()
        result = _run_github_actions_lint(cmd_mox)

        expected_exit = 0 if yamllint_exit == actionlint_exit == 0 else 2
        assert result.returncode == expected_exit, (
            "the `github-actions-lint` target must fail for every non-zero "
            f"linter exit status; stderr was: {result.stderr}"
        )
        if yamllint_exit != 0:
            actionlint.assert_not_called()
        yamllint.assert_called()
