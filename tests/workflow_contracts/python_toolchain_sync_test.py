"""Contract tests keeping the Python toolchain pins in sync.

The Makefile pins the Ruff and ty releases its gates run under, and
``.github/workflows/ci.yml`` re-declares the same pins as workflow ``env``
values (Make's ``?=`` assignments yield to the environment, so the workflow
values control CI). A drifted pair silently runs different rule sets locally
and in CI, which surfaces as version-skew lint failures far from the edit
that caused them.

These tests parse both files and assert the pins agree, without asserting
any specific version: bumping a pin is routine, and must simply happen in
both places in the same commit. The Python baseline receives the same
treatment across the Makefile, the CI workflow, the release workflow, and
the build-and-package workflow default.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

import pytest
from workflow_loading import (
    CI_WORKFLOW_PATH,
    MAKEFILE_PATH,
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    load_workflow,
    require_mapping,
)

if typ.TYPE_CHECKING:
    from pathlib import Path

#: Pins that must agree between the Makefile and the CI workflow env block.
SYNCED_PINS = ("RUFF_VERSION", "TY_VERSION", "PYTHON_BASELINE")


def _makefile_variable(name: str) -> str:
    """Return the default value a ``NAME ?=`` assignment gives in the Makefile."""
    text = MAKEFILE_PATH.read_text(encoding="utf-8")
    pattern = re.compile(rf"^{re.escape(name)} \?= (\S+)$", flags=re.MULTILINE)
    matches = pattern.findall(text)
    assert len(matches) == 1, (
        f"expected exactly one '{name} ?=' assignment in the Makefile, "
        f"found {len(matches)}"
    )
    return matches[0]


def _ci_env_value(name: str) -> str:
    """Return a workflow-level env value from ci.yml."""
    env = require_mapping(
        load_workflow(CI_WORKFLOW_PATH).get("env"),
        "ci.yml workflow-level env block",
    )
    value = env.get(name)
    assert isinstance(value, str), (
        f"ci.yml env must pin {name} as a string, got {value!r}"
    )
    return value


@pytest.mark.parametrize("name", SYNCED_PINS)
def test_ci_env_pin_matches_makefile_default(name: str) -> None:
    """Each toolchain pin in ci.yml equals the Makefile's default.

    The CI env value overrides the Makefile's ``?=`` default, so a drifted
    pair runs different tool versions locally and in CI. No specific version
    is asserted; only agreement is.
    """
    makefile_value = _makefile_variable(name)
    ci_value = _ci_env_value(name)
    assert makefile_value == ci_value, (
        f"{name} must match between the Makefile ({makefile_value!r}) and "
        f"the ci.yml env block ({ci_value!r}); bump both in the same commit"
    )


@pytest.mark.parametrize("name", ["RUFF_VERSION", "TY_VERSION"])
def test_tool_pins_are_exact_versions(name: str) -> None:
    """The Ruff and ty pins are exact dotted versions, not ranges or 'latest'."""
    value = _makefile_variable(name)
    assert re.fullmatch(r"\d+\.\d+\.\d+", value), (
        f"{name} must pin an exact X.Y.Z release so local runs and CI "
        f"resolve identical rule sets, got {value!r}"
    )


def _setup_uv_python_versions(path: Path) -> list[str]:
    """Return every setup-uv ``python-version`` input in a workflow file."""
    versions: list[str] = []
    workflow = load_workflow(path)
    jobs = require_mapping(workflow.get("jobs"), f"{path.name} jobs")
    for job in jobs.values():
        job_mapping = require_mapping(job, f"{path.name} job")
        match job_mapping.get("steps", []):
            case list() as steps:
                pass
            case value:
                pytest.fail(f"{path.name} job steps must be a list, got {value!r}")
        for step in steps:
            step_mapping = require_mapping(step, f"{path.name} setup step")
            match step_mapping.get("uses"):
                case str() as uses if uses.startswith("astral-sh/setup-uv@"):
                    pass
                case _:
                    continue
            with_ = require_mapping(
                step_mapping.get("with"), f"{path.name} setup-uv with block"
            )
            match with_.get("python-version"):
                case str() as version:
                    versions.append(version)
                case None:
                    continue
                case value:
                    pytest.fail(
                        f"{path.name} setup-uv python-version must be a string, "
                        f"got {value!r}"
                    )
    return versions


def test_release_workflow_pins_the_python_baseline() -> None:
    """The release hoist job installs the repository's Python baseline."""
    baseline = _makefile_variable("PYTHON_BASELINE")
    versions = _setup_uv_python_versions(RELEASE_WORKFLOW_PATH)
    assert versions, "release.yml must install a pinned Python via setup-uv"
    assert all(version == baseline for version in versions), (
        f"every setup-uv python-version in release.yml must equal the "
        f"Makefile PYTHON_BASELINE ({baseline!r}), got {versions!r}"
    )
    text = RELEASE_WORKFLOW_PATH.read_text(encoding="utf-8")
    stray = sorted(set(re.findall(r"--python (\S+)", text)) - {baseline})
    assert not stray, (
        f"release.yml passes uv --python versions other than the "
        f"baseline {baseline!r}: {stray!r}"
    )


def test_ci_setup_uv_steps_install_the_python_baseline() -> None:
    """Every setup-uv step in ci.yml that pins a Python pins the baseline.

    The steps reference ``${{ env.PYTHON_BASELINE }}`` rather than a literal,
    so the workflow-level env value (already held equal to the Makefile by
    ``test_ci_env_pin_matches_makefile_default``) is the single source.
    """
    versions = _setup_uv_python_versions(CI_WORKFLOW_PATH)
    assert versions, "ci.yml must install a pinned Python via setup-uv"
    literal = [v for v in versions if v != "${{ env.PYTHON_BASELINE }}"]
    assert not literal, (
        f"setup-uv python-version inputs in ci.yml must reference "
        f"${{{{ env.PYTHON_BASELINE }}}} so the pin has one source, "
        f"got literals {literal!r}"
    )


def test_package_workflow_default_matches_the_python_baseline() -> None:
    """The build-and-package python-version input defaults to the baseline."""
    baseline = _makefile_variable("PYTHON_BASELINE")
    workflow = load_workflow(PACKAGE_WORKFLOW_PATH)
    triggers = require_mapping(workflow.get("on"), "build-and-package.yml on mapping")
    workflow_call = require_mapping(
        triggers.get("workflow_call"), "build-and-package.yml workflow_call mapping"
    )
    inputs = require_mapping(
        workflow_call.get("inputs"), "build-and-package.yml workflow_call inputs"
    )
    python_version = require_mapping(
        inputs.get("python-version"), "build-and-package.yml python-version input"
    )
    default = python_version.get("default")
    assert default == baseline, (
        f"build-and-package.yml python-version default must equal the "
        f"Makefile PYTHON_BASELINE ({baseline!r}), got {default!r}"
    )


def _makefile_target(target: str) -> tuple[list[str], str]:
    """Return one Make target's prerequisites and complete recipe text."""
    text = MAKEFILE_PATH.read_text(encoding="utf-8")
    match = re.search(
        rf"^{re.escape(target)}:([^\n#]*)(?:\s+##[^\n]*)?\n((?:\t[^\n]*\n?)*)",
        text,
        flags=re.MULTILINE,
    )
    assert match is not None, f"the Makefile must define the {target} target"
    prerequisites = match.group(1).split()
    return prerequisites, match.group(2)


def test_python_quality_targets_preserve_their_dependency_graph() -> None:
    """The Rust umbrella gates depend on their corresponding Python gates."""
    lint_prerequisites, _ = _makefile_target("lint")
    typecheck_prerequisites, _ = _makefile_target("typecheck")

    assert "lint-python" in lint_prerequisites, (
        "make lint must depend on lint-python so Python lint failures block CI"
    )
    assert "typecheck-python" in typecheck_prerequisites, (
        "make typecheck must depend on typecheck-python so ty failures block CI"
    )


def test_python_quality_targets_run_the_pinned_local_commands() -> None:
    """Local Make targets invoke each pinned Python formatter, linter, and typer."""
    expected_commands = {
        "lint-python": (
            "$(RUFF) check $(PYTHON_SOURCES)",
            "$(PYLINT) $(PYLINT_TARGETS)",
            "$(DF12_PYLINT) $(PYLINT_TARGETS)",
            "$(AMBRLEAKS) $(PYTHON_SOURCES)",
        ),
        "typecheck-python": (
            "ty check --python-version $(PYTHON_BASELINE)",
            "--extra-search-path scripts $(PYTHON_SOURCES)",
        ),
        "fmt": ("$(RUFF) format $(PYTHON_SOURCES)",),
        "check-fmt": ("$(RUFF) format --check $(PYTHON_SOURCES)",),
    }
    for target, commands in expected_commands.items():
        _, recipe = _makefile_target(target)
        missing = [command for command in commands if command not in recipe]
        assert not missing, (
            f"{target} must preserve its pinned Python command wiring; "
            f"missing {missing!r} from {recipe!r}"
        )
