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
from pathlib import Path

import yaml

if typ.TYPE_CHECKING:
    from cmd_mox import CmdMox


pytest_plugins = ("cmd_mox.pytest_plugin",)


REPO_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE_PATH = REPO_ROOT / "Makefile"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "ci.yml"
YAMLLINT_POLICY_PATH = REPO_ROOT / ".yamllint.yml"
WORKFLOW_DIR = REPO_ROOT / ".github" / "workflows"

ACTIONLINT_VERSION = "1.7.12"
ACTIONLINT_SHA256 = "8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8"
ACTIONLINT_INSTALLER_COMMIT = "914e7df21a07ef503a81201c76d2b11c789d3fca"
ACTIONLINT_ARCHIVE = f"actionlint_{ACTIONLINT_VERSION}_linux_x86_64.tar.gz"
ACTIONLINT_ARCHIVE_VARIABLE = "ACTIONLINT_ARCHIVE"


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


def _build_test_steps() -> list[dict[str, object]]:
    """Return the Linux CI job's steps from the workflow declaration."""
    workflow = yaml.safe_load(_read(WORKFLOW_PATH))
    assert isinstance(workflow, dict), "CI workflow must parse to a mapping"
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "CI workflow must define its jobs"
    build_test = jobs.get("build-test")
    assert isinstance(build_test, dict), (
        "CI workflow must define the Linux build-test job"
    )
    steps = build_test.get("steps")
    assert isinstance(steps, list), "the Linux build-test job must define steps"
    assert all(isinstance(step, dict) for step in steps), (
        "the Linux build-test job's steps must be mappings"
    )
    return steps


def _step(steps: list[dict[str, object]], name: str) -> dict[str, object]:
    """Return the uniquely named CI step required by a workflow contract."""
    matches = [step for step in steps if step.get("name") == name]
    assert len(matches) == 1, f"CI must define exactly one `{name}` step"
    return matches[0]


def _step_position(steps: list[dict[str, object]], name: str) -> int:
    """Return a required CI step's position for ordering assertions."""
    return steps.index(_step(steps, name))


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
        [
            "/usr/bin/make",
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

    assert "lint: lint-clippy lint-whitaker github-actions-lint" in makefile, (
        "the `lint` target must delegate workflow validation to `github-actions-lint`"
    )
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
    steps = _build_test_steps()
    setup_uv = _step(steps, "Setup uv")
    cache_yamllint = _step(steps, "Cache yamllint")
    install_yamllint = _step(steps, "Install yamllint")
    cache_actionlint = _step(steps, "Cache actionlint")
    download_actionlint = _step(steps, "Download actionlint")
    lint = _step(steps, "Lint")
    download_script = download_actionlint.get("run")

    assert setup_uv.get("uses") == (
        "astral-sh/setup-uv@11f9893b081a58869d3b5fccaea48c9e9e46f990"
    ), "the Linux CI job must provision uv before installing yamllint"
    assert cache_yamllint.get("uses") == (
        "actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9"
    ), "the Linux CI job must cache the pinned yamllint installation"
    assert install_yamllint.get("run") == (
        'uv tool install "yamllint==${YAMLLINT_VERSION}"\n'
        'echo "${UV_TOOL_BIN_DIR}" >> "$GITHUB_PATH"\n'
    ), "the Linux CI job must install and expose the pinned yamllint binary"
    assert cache_actionlint.get("id") == "cache_actionlint", (
        "the actionlint cache step must expose its cache-hit result"
    )
    assert cache_actionlint.get("with") == {
        "path": "actionlint",
        "key": "actionlint-${{ runner.os }}-${{ runner.arch }}-1.7.12",
    }, "the actionlint cache must own the pinned binary at the repository path"
    assert download_actionlint.get("if") == (
        "steps.cache_actionlint.outputs.cache-hit != 'true'"
    ), "the actionlint download must run only after a cache miss"
    assert isinstance(download_script, str), (
        "the actionlint cache-miss step must define its verified installer script"
    )
    assert f"readonly ACTIONLINT_VERSION='{ACTIONLINT_VERSION}'" in download_script, (
        "the actionlint installer must pin the expected release version"
    )
    assert f"readonly ACTIONLINT_SHA256='{ACTIONLINT_SHA256}'" in download_script, (
        "the actionlint installer must pin the expected release archive checksum"
    )
    assert (
        f"readonly ACTIONLINT_INSTALLER_COMMIT='{ACTIONLINT_INSTALLER_COMMIT}'"
        in download_script
    ), "the actionlint installer must pin its reviewed installer revision"
    assert f'readonly ACTIONLINT_ARCHIVE="{ACTIONLINT_ARCHIVE}"' in download_script, (
        "the actionlint installer must request the published Linux x86-64 archive"
    )
    assert (
        'readonly ACTIONLINT_RELEASE_URL="$'
        "{ACTIONLINT_RELEASE_BASE}/"
        "$"
        f"{{{ACTIONLINT_ARCHIVE_VARIABLE}}}"
        '"'
    ) in download_script, (
        "the actionlint installer must derive the release URL from its pinned archive"
    )
    assert download_script.index("sha256sum --check --") < download_script.index(
        'bash "${ACTIONLINT_INSTALLER_PATH}" "${ACTIONLINT_VERSION}"'
    ), "the actionlint archive checksum must be verified before running the installer"
    assert lint.get("run") == (
        '/usr/bin/make ACTIONLINT="$GITHUB_WORKSPACE/actionlint" lint'
    ), "the Linux CI job must use trusted `/usr/bin/make` with the cached actionlint"
    assert all(
        _step_position(steps, earlier) < _step_position(steps, later)
        for earlier, later in [
            ("Setup uv", "Cache yamllint"),
            ("Cache yamllint", "Install yamllint"),
            ("Install yamllint", "Cache actionlint"),
            ("Cache actionlint", "Download actionlint"),
            ("Download actionlint", "Lint"),
        ]
    ), "the Linux CI job must provision both linters before the trusted lint step"


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
