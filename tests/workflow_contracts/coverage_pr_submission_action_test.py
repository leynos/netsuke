"""Exercise the pure Python outcome seam used by trusted coverage reporting.

The GitHub workflow invokes this checked-in Python module after hostile-data
validation. Loading it directly keeps the outcome decision executable without
workflow event data or network access.
"""

import functools
import importlib.util
import sys
import typing as typ

import pytest
from workflow_loading import REPO_ROOT

if typ.TYPE_CHECKING:
    import types

ACTION_MODULE_PATH = REPO_ROOT / ".github" / "scripts" / "coverage_pr_submission.py"


@functools.cache
def _coverage_action_module() -> types.ModuleType:
    """Load the checked-in trusted coverage Python action module."""
    specification = importlib.util.spec_from_file_location(
        "coverage_pr_submission", ACTION_MODULE_PATH
    )
    assert specification is not None, "the trusted action module must be loadable"
    assert specification.loader is not None, (
        "the trusted action module must provide a loader"
    )
    module = importlib.util.module_from_spec(specification)
    sys.modules[specification.name] = module
    specification.loader.exec_module(module)
    return module


@pytest.mark.parametrize(
    ("download_outcome", "validation_outcome", "submission_outcome", "expected"),
    [
        pytest.param("success", "success", "success", "success", id="success"),
        pytest.param("success", "success", "skipped", "neutral", id="absent-token"),
        pytest.param("failure", "skipped", "skipped", "failure", id="download-failure"),
        pytest.param(
            "success", "failure", "skipped", "failure", id="validation-failure"
        ),
        pytest.param(
            "success", "success", "failure", "failure", id="submission-failure"
        ),
    ],
)
def test_coverage_conclusion_preserves_stage_outcomes(
    download_outcome: str,
    validation_outcome: str,
    submission_outcome: str,
    expected: str,
) -> None:
    """Map every trusted-handoff terminal state to its Check Run conclusion."""
    action_module = _coverage_action_module()
    assert (
        action_module.coverage_conclusion(
            download_outcome, validation_outcome, submission_outcome
        )
        == expected
    ), "the Check Run conclusion must match its three stage outcomes"
