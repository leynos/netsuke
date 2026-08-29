"""Contract tests for the GitHub Actions validation toolchain.

The workflow linters are installed only on the Linux CI job and run through
the Makefile's ``lint`` target. These checks keep the YAML policy, trusted
tool provisioning, and Makefile wiring in step without testing either
third-party linter's own behaviour.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE_PATH = REPO_ROOT / "Makefile"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "ci.yml"
YAMLLINT_POLICY_PATH = REPO_ROOT / ".yamllint.yml"
WORKFLOW_DIR = REPO_ROOT / ".github" / "workflows"

ACTIONLINT_VERSION = "1.7.12"
ACTIONLINT_SHA256 = "8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8"
ACTIONLINT_INSTALLER_COMMIT = "914e7df21a07ef503a81201c76d2b11c789d3fca"


def _read(path: Path) -> str:
    """Read a repository contract file as UTF-8 text."""
    return path.read_text(encoding="utf-8")


def test_makefile_runs_both_github_actions_linters() -> None:
    """The lint target delegates workflow validation to both linters."""
    makefile = _read(MAKEFILE_PATH)

    assert "lint: lint-clippy lint-whitaker github-actions-lint" in makefile
    assert "github-actions-lint: ## Validate GitHub Actions workflows" in makefile
    assert "$(YAMLLINT) --config-file .yamllint.yml .github/workflows" in makefile
    assert "$(ACTIONLINT)" in makefile


def test_yamllint_policy_supports_github_actions_workflows() -> None:
    """The YAML policy accepts GitHub's trigger keys and workflow line lengths."""
    policy = _read(YAMLLINT_POLICY_PATH)

    assert "extends: default" in policy
    assert "max: 120" in policy
    assert "allowed-values: ['true', 'false']" in policy
    assert "check-keys: false" in policy


def test_ci_installs_and_invokes_pinned_workflow_linters() -> None:
    """Linux CI provisions the pinned tools before the trusted lint invocation."""
    workflow = _read(WORKFLOW_PATH)

    assert "YAMLLINT_VERSION: '1.38.0'" in workflow
    assert "- name: Setup uv" in workflow
    assert "- name: Cache yamllint" in workflow
    assert 'uv tool install "yamllint==${YAMLLINT_VERSION}"' in workflow
    assert "- name: Cache actionlint" in workflow
    assert "id: cache_actionlint" in workflow
    assert "if: steps.cache_actionlint.outputs.cache-hit != 'true'" in workflow
    assert f"readonly ACTIONLINT_VERSION='{ACTIONLINT_VERSION}'" in workflow
    assert f"readonly ACTIONLINT_SHA256='{ACTIONLINT_SHA256}'" in workflow
    assert (
        f"readonly ACTIONLINT_INSTALLER_COMMIT='{ACTIONLINT_INSTALLER_COMMIT}'"
        in workflow
    )
    assert 'run: /usr/bin/make ACTIONLINT="$GITHUB_WORKSPACE/actionlint" lint' in workflow
    assert "PATH=\"$GITHUB_WORKSPACE:$PATH\"" not in workflow


def test_every_workflow_starts_a_yaml_document() -> None:
    """Every checked workflow declares the document start required by the policy."""
    workflow_paths = sorted(WORKFLOW_DIR.glob("*.yml"))

    assert workflow_paths, "the repository must contain GitHub Actions workflows"
    for workflow_path in workflow_paths:
        assert _read(workflow_path).startswith("---\n"), (
            f"{workflow_path.relative_to(REPO_ROOT)} must begin with a YAML document start"
        )
