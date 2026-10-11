"""Contract tests for the strict workflow loader.

Every workflow contract in this directory reads workflows through
``workflow_loading``, so its two promises are held here: a repeated mapping key
is refused instead of silently keeping the last value, and distinct keys that
YAML 1.1 would conflate stay apart.

Run via ``make test-workflow-contracts``.
"""

import pytest
from workflow_loading import WorkflowReadError, parse_workflow_text


def test_the_loader_refuses_a_repeated_key() -> None:
    """Fail on `runs-on` declared twice rather than keeping the second label.

    PyYAML's default is to keep the last value silently, so a paid label in
    the discarded half would read as hosted to every placement contract.
    """
    doubled = (
        "on: pull_request\n"
        "jobs:\n"
        "  build:\n"
        "    runs-on: ubicloud-standard-4-ubuntu-2404\n"
        "    runs-on: ubuntu-latest\n"
    )
    with pytest.raises(WorkflowReadError, match="runs-on"):
        parse_workflow_text(doubled, "doubled workflow")


def test_the_loader_keeps_distinct_keys() -> None:
    """Keep `on` and a boolean-looking key apart, so only real repeats fail."""
    parsed = parse_workflow_text("on: push\n'true': 1\njobs: {}\n", "workflow")
    assert parsed == {"on": "push", "true": 1, "jobs": {}}, (
        f"distinct keys must survive the strict loader, got {parsed!r}"
    )
