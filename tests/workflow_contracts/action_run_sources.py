"""Locate GitHub expression source text in the ``run`` blocks of this tree.

Values written into a ``run`` block are shell or PowerShell source, so a
GitHub expression there is executable code rather than a substituted value.
The same hazard exists in a workflow job and in a composite action, because a
composite action is a step body that has been moved into its own file, and
moving it does not move it out of harm's way.

This module holds the predicate and the two containers it reads -- workflow
jobs and composite-action manifests -- so one rule serves both and neither copy
can drift. It carries no tests of its own; the suites that exercise it are
``release_run_interpolation_test`` and ``composite_action_contract_test``.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
import yaml
from workflow_loading import REPO_ROOT, require_list, require_mapping

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    import pathlib

#: The introducer of a GitHub expression.
EXPRESSION = "${{"

#: Where same-repository composite actions live. GitHub looks for a manifest
#: directly inside each action directory, under either extension.
ACTION_DIRECTORY = ".github/actions"

#: The manifest file names GitHub accepts for an action.
ACTION_MANIFEST_NAMES = ("action.yml", "action.yaml")


def step_run_violations(
    source_name: str, steps: cabc.Sequence[dict[str, object]]
) -> list[str]:
    """Describe ``run`` blocks containing expression source text.

    The steps carry no job identifier of their own, so ``source_name`` labels
    the diagnostic and is the file the steps came from.

    Parameters
    ----------
    source_name : str
        Label for the diagnostic, naming the file the steps came from.
    steps : cabc.Sequence[dict[str, object]]
        Step mappings to inspect, in declaration order.

    Returns
    -------
    list[str]
        One diagnostic for each run block containing expression source text.

    Examples
    --------
    >>> step_run_violations("demo.yml", [{"name": "Tag", "run": "echo ${{ tag }}"}])
    ["demo.yml step 0 (Tag): run block contains '${{'"]
    >>> step_run_violations("demo.yml", [{"name": "Tag", "run": 'echo "$TAG"'}])
    []
    """
    violations = []
    for index, step in enumerate(steps):
        run = step.get("run")
        if isinstance(run, str) and EXPRESSION in run:
            step_name = str(step.get("name", "<unnamed>"))
            violations.append(
                f"{source_name} step {index} ({step_name}): "
                f"run block contains '{EXPRESSION}'"
            )
    return violations


def workflow_step_contexts(
    workflow_name: str, workflow: dict[str, object]
) -> cabc.Iterator[tuple[str, int, dict[str, object]]]:
    """Yield every step from jobs that define step lists.

    Jobs calling another workflow have ``uses`` and no step list. Every other
    job must provide a list, including jobs with no run blocks, so an invalid
    shape cannot evade the contracts that read this.

    Parameters
    ----------
    workflow_name : str
        Label for diagnostics and for the shape errors raised here.
    workflow : dict[str, object]
        Parsed workflow document whose ``jobs`` are walked.

    Yields
    ------
    tuple[str, int, dict[str, object]]
        Each job ID, step index, and step mapping.
    """
    jobs = require_mapping(workflow.get("jobs"), f"{workflow_name} jobs")
    for job_id, raw_job in jobs.items():
        job = require_mapping(raw_job, f"{workflow_name} job {job_id}")
        if "uses" in job:
            continue
        steps = job.get("steps")
        assert isinstance(steps, list), (
            f"{workflow_name} job {job_id} must declare steps as a list"
        )
        for index, raw_step in enumerate(steps):
            step = require_mapping(
                raw_step, f"{workflow_name} job {job_id} step {index}"
            )
            yield str(job_id), index, step


def run_expression_violations(
    workflow_name: str, workflow: dict[str, object]
) -> list[str]:
    """Describe workflow run blocks containing GitHub expression source text.

    Parameters
    ----------
    workflow_name : str
        Label for the diagnostic, naming the workflow file.
    workflow : dict[str, object]
        Parsed workflow document whose job steps are inspected.

    Returns
    -------
    list[str]
        One diagnostic for each run block containing expression source text.

    Examples
    --------
    >>> workflow = {
    ...     "jobs": {
    ...         "publish": {"steps": [{"name": "Tag", "run": "echo ${{ tag }}"}]}
    ...     }
    ... }
    >>> run_expression_violations("release.yml", workflow)
    ["release.yml job publish step 0 (Tag): run block contains '${{'"]
    """
    violations = []
    for job_id, index, step in workflow_step_contexts(workflow_name, workflow):
        run = step.get("run")
        if isinstance(run, str) and EXPRESSION in run:
            step_name = str(step.get("name", "<unnamed>"))
            violations.append(
                f"{workflow_name} job {job_id} step {index} ({step_name}): "
                f"run block contains '{EXPRESSION}'"
            )
    return violations


def _is_action_directory(path: pathlib.Path) -> bool:
    """Report whether ``path`` is a real action directory rather than a link.

    A symlinked action directory is skipped for the same reason its manifest
    is: its content is not this tree's to vouch for, so a manifest reached
    through the link would assert a rule about code held elsewhere.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as root:
    ...     _ = (Path(root) / "real").mkdir()
    ...     _ = (Path(root) / "linked").symlink_to(Path(root) / "real")
    ...     _is_action_directory(Path(root) / "real")
    ...     _is_action_directory(Path(root) / "linked")
    True
    False
    """
    return path.is_dir() and not path.is_symlink()


def _is_action_manifest(path: pathlib.Path) -> bool:
    """Report whether ``path`` is a real manifest file rather than a link.

    Both halves are required: an absent ``action.yaml`` and a linked one are
    different facts, and only the first means the action does not carry one.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as root:
    ...     manifest = Path(root) / "action.yml"
    ...     _ = manifest.write_text("runs:\\n", encoding="utf-8")
    ...     _is_action_manifest(manifest)
    ...     _is_action_manifest(Path(root) / "action.yaml")
    True
    False
    """
    return path.is_file() and not path.is_symlink()


def action_manifest_paths(
    repository_root: pathlib.Path = REPO_ROOT,
) -> list[pathlib.Path]:
    """Return every composite action manifest under ``.github/actions``.

    Mirrors ``local_reference_inventory._discover_action_manifests``: a manifest
    is ``action.yml`` or ``action.yaml`` directly inside an action directory.
    Symlinks are skipped there for the same reason they are skipped here -- a
    linked manifest's content is not this tree's to vouch for.

    Parameters
    ----------
    repository_root : pathlib.Path
        Repository root holding the action directory.

    Returns
    -------
    list[pathlib.Path]
        Each manifest path, ordered by action directory then manifest name.
    """
    action_root = repository_root / ACTION_DIRECTORY
    directories = sorted(
        directory
        for directory in action_root.iterdir()
        if _is_action_directory(directory)
    )
    return [
        manifest
        for directory in directories
        for manifest in (directory / name for name in ACTION_MANIFEST_NAMES)
        if _is_action_manifest(manifest)
    ]


def action_sources(
    repository_root: pathlib.Path = REPO_ROOT,
) -> dict[str, dict[str, object]]:
    """Load every composite action manifest into a repository-relative mapping.

    Parameters
    ----------
    repository_root : pathlib.Path
        Repository root holding the action directory.

    Returns
    -------
    dict[str, dict[str, object]]
        Each manifest's parsed document, keyed by its path relative to the
        repository root so a diagnostic names the file a reader can open.
    """
    return {
        path.relative_to(repository_root).as_posix(): require_mapping(
            yaml.safe_load(path.read_text(encoding="utf-8")), path.name
        )
        for path in action_manifest_paths(repository_root)
    }


def action_steps(
    action_name: str, document: dict[str, object]
) -> list[dict[str, object]]:
    """Return one composite action's ``runs.steps`` as mappings.

    Parameters
    ----------
    action_name : str
        Label for the shape errors raised while reading the steps.
    document : dict[str, object]
        Parsed composite-action manifest.

    Returns
    -------
    list[dict[str, object]]
        Each step mapping, in declaration order.
    """
    runs = require_mapping(document.get("runs"), f"{action_name} runs")
    return [
        require_mapping(step, f"{action_name} step {index}")
        for index, step in enumerate(
            require_list(runs.get("steps"), f"{action_name} steps")
        )
    ]


def action_run_violations(action_name: str, document: dict[str, object]) -> list[str]:
    """Describe run blocks in one composite action containing expressions.

    Parameters
    ----------
    action_name : str
        Label for the diagnostic, naming the action manifest.
    document : dict[str, object]
        Parsed composite-action manifest whose steps are inspected.

    Returns
    -------
    list[str]
        One diagnostic for each run block containing expression source text.

    Examples
    --------
    >>> document = {
    ...     "runs": {"steps": [{"name": "Tag", "run": "echo ${{ tag }}"}]}
    ... }
    >>> action_run_violations("demo/action.yml", document)
    ["demo/action.yml step 0 (Tag): run block contains '${{'"]
    """
    return step_run_violations(action_name, action_steps(action_name, document))


def require_action_step(
    action_name: str, document: dict[str, object], step_name: str
) -> int:
    """Return the index of one named step, failing when it is absent.

    A mutation that restores an unsafe body needs the location it restores it
    to. Returning the index keeps the caller from re-deriving it and silently
    asserting on ``-1`` when the name has moved.

    Parameters
    ----------
    action_name : str
        Label for the shape errors and for the absence diagnostic.
    document : dict[str, object]
        Parsed composite-action manifest to search.
    step_name : str
        The ``name`` the step must carry.

    Returns
    -------
    int
        The declaration index of the named step.
    """
    for index, step in enumerate(action_steps(action_name, document)):
        if step.get("name") == step_name:
            return index
    # `return` rather than a bare call: it keeps every exit from this function
    # an expression, which the house lint requires. `pytest.fail` never
    # returns, so the annotation still holds.
    return pytest.fail(f"{action_name} has no step named {step_name!r}")
