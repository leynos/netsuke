"""Shared GitHub Actions workflow parsing for the CI contract suites.

``ci_lint_test.py``, ``ci_windows_job_test.py``, and
``ci_coverage_wiring_test.py`` all read the same workflow files and need the
same guarantees before they can assert anything: the document parses to a
string-keyed mapping, the jobs it declares are mappings, and no job shadows
the workflow-scoped ``NEXTEST_VERSION`` pin. Those checks live here so each
suite states its contract rather than restating the parsing preamble, and so
the YAML 1.2 boolean workaround has a single home.

Run via ``make test-workflow-contracts``.
"""

import re
from pathlib import Path

import pytest
import yaml

REPO_ROOT = Path(__file__).resolve().parents[2]
CI_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "ci.yml"
COVERAGE_MAIN_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "coverage-main.yml"
MAKEFILE_PATH = REPO_ROOT / "Makefile"


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
        (tag, regexp) for tag, regexp in resolvers if tag != "tag:yaml.org,2002:bool"
    ]
    for initial, resolvers in yaml.SafeLoader.yaml_implicit_resolvers.items()
}
_WorkflowLoader.add_implicit_resolver(
    "tag:yaml.org,2002:bool",
    re.compile(r"^(?:true|True|TRUE|false|False|FALSE)$"),
    list("tTfF"),
)


def require_mapping(value: object, description: str) -> dict[str, object]:
    """Return ``value`` as a mapping, failing the test when it is not one."""
    match value:
        case dict() as mapping:
            return mapping
        case other:
            pytest.fail(f"{description} must be a mapping, got {type(other).__name__}")


def _require_string_keys(mapping: dict[str, object], description: str) -> None:
    """Fail unless every key of ``mapping`` is a string."""
    offenders = sorted(repr(key) for key in mapping if not isinstance(key, str))
    if offenders:
        pytest.fail(f"{description} must be string-keyed, got {offenders}")


def _require_no_nextest_override(name: str, mapping: dict[str, object]) -> None:
    """Fail when a job redeclares the workflow-scoped ``NEXTEST_VERSION`` pin."""
    if "env" not in mapping:
        return
    env = require_mapping(mapping["env"], f"jobs.{name}.env")
    if "NEXTEST_VERSION" in env:
        pytest.fail(f"jobs.{name}.env must not redeclare NEXTEST_VERSION")


def load_workflow(workflow_path: Path = CI_WORKFLOW_PATH) -> dict[str, object]:
    """Parse a workflow file, rejecting anything but a well-formed mapping root.

    A YAML document happily parses to ``None`` when empty, or to a scalar or
    list when malformed. Without a runtime check the return annotation is a
    claim rather than a guarantee, and the failure surfaces later as an opaque
    ``AttributeError`` far from the real cause.

    Parameters
    ----------
    workflow_path
        Workflow file to parse; defaults to ``.github/workflows/ci.yml``.

    Returns
    -------
    dict[str, object]
        The parsed, string-keyed workflow mapping.
    """
    # The loader is driven directly rather than through `yaml.load` because
    # `_WorkflowLoader` derives from `SafeLoader` and constructs no arbitrary
    # Python objects; going through `yaml.load` would only obscure that.
    loader = _WorkflowLoader(workflow_path.read_text(encoding="utf-8"))
    try:
        document = loader.get_single_data()
    finally:
        loader.dispose()

    workflow = require_mapping(document, "the workflow")
    _require_string_keys(workflow, "the workflow mapping")
    jobs = require_mapping(workflow.get("jobs"), "the workflow jobs")
    _require_string_keys(jobs, "the jobs mapping")
    for name, declaration in jobs.items():
        _require_no_nextest_override(name, require_mapping(declaration, f"jobs.{name}"))
    return workflow


def workflow_job(workflow: dict[str, object], name: str) -> dict[str, object]:
    """Return the named job's mapping."""
    jobs = require_mapping(workflow.get("jobs"), "the workflow jobs")
    return require_mapping(jobs.get(name), f"the {name} job")


def job_steps(workflow: dict[str, object], name: str) -> list[dict[str, object]]:
    """Return the named job's steps, in declaration order."""
    match workflow_job(workflow, name).get("steps"):
        case list() as steps:
            return steps
        case _:
            pytest.fail(f"jobs.{name}.steps must be a list")


def named_step(steps: list[dict[str, object]], name: str) -> dict[str, object]:
    """Return the single step called ``name``, failing on any other count."""
    matches = [step for step in steps if step.get("name") == name]
    assert len(matches) == 1, (
        f"expected exactly one step named {name!r}, found {len(matches)}"
    )
    return matches[0]


def step_runs(steps: list[dict[str, object]]) -> list[object]:
    """Return the ``run`` script of every step, preserving ``None`` for actions."""
    return [step.get("run") for step in steps]


def step_uses(steps: list[dict[str, object]]) -> list[str]:
    """Return the ``uses`` reference of every step as a string."""
    return [str(step.get("uses", "")) for step in steps]
