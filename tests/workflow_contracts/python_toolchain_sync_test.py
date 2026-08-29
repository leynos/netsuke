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
from pathlib import Path

import pytest
import yaml

REPO_ROOT = Path(__file__).resolve().parents[2]
MAKEFILE_PATH = REPO_ROOT / "Makefile"
CI_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "ci.yml"
RELEASE_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "release.yml"
PACKAGE_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "build-and-package.yml"

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


def _workflow(path: Path) -> dict[str, object]:
    """Parse a workflow file."""
    return yaml.safe_load(path.read_text(encoding="utf-8"))


def _ci_env_value(name: str) -> str:
    """Return a workflow-level env value from ci.yml."""
    env = _workflow(CI_WORKFLOW_PATH).get("env")
    assert isinstance(env, dict), "ci.yml must declare a workflow-level env block"
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
    workflow = _workflow(path)
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), f"{path.name} must declare a jobs mapping"
    for job in jobs.values():
        for step in job.get("steps", []):
            uses = step.get("uses", "")
            if not uses.startswith("astral-sh/setup-uv@"):
                continue
            version = (step.get("with") or {}).get("python-version")
            if version is not None:
                versions.append(version)
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
    workflow = _workflow(PACKAGE_WORKFLOW_PATH)
    # PyYAML resolves the bare ``on:`` key as the boolean ``True``.
    triggers = workflow.get("on", workflow.get(True))
    assert isinstance(triggers, dict), (
        "build-and-package.yml must declare an on: mapping"
    )
    inputs = triggers.get("workflow_call", {}).get("inputs", {})
    default = inputs.get("python-version", {}).get("default")
    assert default == baseline, (
        f"build-and-package.yml python-version default must equal the "
        f"Makefile PYTHON_BASELINE ({baseline!r}), got {default!r}"
    )
