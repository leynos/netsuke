"""Keep release workflow run blocks free of GitHub expression source text.

Values in ``run`` blocks are shell or PowerShell source, so GitHub expressions
there become executable code. These contracts require the workflows to pass
those values through step environments instead. Each bounded mutation restores
one formerly unsafe step, and a second pass injects an expression into every
current run step individually.

Run via ``make test-workflow-contracts``.
"""

import copy
import typing as typ

import pytest
from workflow_loading import (
    PACKAGE_WORKFLOW_PATH,
    RELEASE_WORKFLOW_PATH,
    load_workflow,
    require_mapping,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

EXPRESSION = "${{"

# Keep each historical source expression in the step where it was expanded.
# The test treats each whole step as one violation, even where the old source
# repeated the expression several times.
ORIGINAL_RUNS = [
    pytest.param(
        "release.yml",
        "metadata",
        "Resolve WiX extension version",
        r"""set -euo pipefail
version=''
if [ "${{ github.event_name }}" = 'workflow_call' ]; then
  version="$(jq -r '.inputs[\"wix-extension-version\"] // empty' \\
    "$GITHUB_EVENT_PATH")"
fi
if [ -z "$version" ] || [ "$version" = 'null' ]; then
  version='7'
fi
echo "value=$version" >>"$GITHUB_OUTPUT"
""",
        id="wix-extension-version",
    ),
    pytest.param(
        "release.yml",
        "release-admission-canaries",
        "Summarise release-admission metrics",
        r"""{
  echo '## Release-admission gate'
  echo
  echo "- Outcome: \`${{ steps.release_admission.outputs.gate-outcome }}\`"
  echo "- Category: \`${{ steps.release_admission.outputs.gate-error-category }}\`"
} >>"$GITHUB_STEP_SUMMARY"
""",
        id="release-admission-summary",
    ),
    pytest.param(
        "release.yml",
        "release",
        "Ensure release exists (draft)",
        r"""set -euo pipefail
gh release view "${{ github.ref_name }}" >/dev/null 2>&1 || \\
  gh release create "${{ github.ref_name }}" \\
    --draft \\
    --verify-tag \\
    --notes "Automated release for ${{ github.ref_name }}"
""",
        id="draft-release-tag",
    ),
    pytest.param(
        "release.yml",
        "release",
        "Hoist cargo-binstall archives to the release root",
        r"""set -euo pipefail
uv run --no-project --python 3.14 \\
  scripts/hoist_binstall_archives.py \\
  --version '${{ needs.metadata.outputs.version }}'
""",
        id="hoist-version",
    ),
    pytest.param(
        "release.yml",
        "release",
        "Check asset upload errors",
        r"""echo "Error uploading release assets:"
printf '%s\\n' "${{ steps.upload_assets.outputs.error-message }}"
exit 1
""",
        id="upload-error-message",
    ),
    pytest.param(
        "build-and-package.yml",
        "build",
        "Report the glibc floor",
        r"""set -euo pipefail
binary="target/${{ inputs.target }}/release/${BIN_NAME}"
floor="$(
  readelf --version-info "${binary}" |
    awk '
      /^Version needs section/ { needs = 1; next }
      /^Version [a-z]+ section/ { needs = 0 }
      needs
    ' |
    grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -n 1
)"
echo "glibc floor for ${{ inputs.target }}: ${floor}"
echo "- glibc \`${{ inputs.target }}\`: \`${floor}\`" >> "$GITHUB_STEP_SUMMARY"
""",
        id="glibc-floor-target",
    ),
    pytest.param(
        "build-and-package.yml",
        "build",
        "Generate release help",
        r"""set -euo pipefail
bash scripts/generate-release-help.sh \\
  "${{ inputs.target }}" \\
  "${{ env.BIN_NAME }}" \\
  "target/orthohelp/${{ inputs.target }}/release" \\
  "${{ inputs.platform == 'windows' && 'Netsuke' || env.BIN_NAME }}"
""",
        id="release-help-inputs",
    ),
    pytest.param(
        "build-and-package.yml",
        "build",
        "Capture staged paths",
        r"""set -euo pipefail
{
  echo "binary_path=${{ steps.stage.outputs['binary-path'] }}"
  echo "man_path=${{ steps.stage.outputs['man-path'] }}"
  echo "license_path=${{ steps.stage.outputs['license-path'] }}"
  echo "artefact_dir=${{ steps.stage.outputs['artifact-dir'] }}"
  echo "powershell_help_dir=${{ steps.stage.outputs.powershell_help_dir }}"
} >>"$GITHUB_OUTPUT"
""",
        id="capture-staged-paths",
    ),
]


def _workflow_sources() -> dict[str, dict[str, object]]:
    """Load both workflows under test."""
    return {
        RELEASE_WORKFLOW_PATH.name: load_workflow(RELEASE_WORKFLOW_PATH),
        PACKAGE_WORKFLOW_PATH.name: load_workflow(PACKAGE_WORKFLOW_PATH),
    }


def _step_contexts(
    workflow_name: str, workflow: dict[str, object]
) -> cabc.Iterator[tuple[str, int, dict[str, object]]]:
    """Yield every step from jobs that define step lists.

    Jobs calling another workflow have ``uses`` and no step list. Every other
    job must provide a list, including jobs with no run blocks, so an invalid
    shape cannot evade this contract.

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
    """Describe run blocks containing GitHub expression source text.

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
    for job_id, index, step in _step_contexts(workflow_name, workflow):
        run = step.get("run")
        if isinstance(run, str) and EXPRESSION in run:
            step_name = str(step.get("name", "<unnamed>"))
            violations.append(
                f"{workflow_name} job {job_id} step {index} ({step_name}): "
                "run block contains '${{'"
            )
    return violations


def _find_step(
    workflow: dict[str, object], job_id: str, step_name: str
) -> dict[str, object]:
    """Return one named step from a non-reusable job."""
    jobs = require_mapping(workflow.get("jobs"), "workflow jobs")
    job = require_mapping(jobs.get(job_id), f"workflow job {job_id}")
    steps = job.get("steps")
    assert isinstance(steps, list), f"workflow job {job_id} must have a steps list"
    for index, raw_step in enumerate(steps):
        step = require_mapping(raw_step, f"workflow job {job_id} step {index}")
        if step.get("name") == step_name:
            return step
    return pytest.fail(f"workflow job {job_id} has no step named {step_name!r}")


def _restore_capture_step(
    workflow: dict[str, object],
    job_id: str,
    run: str,
) -> None:
    """Insert the removed capture step into a workflow mutant."""
    jobs = require_mapping(workflow.get("jobs"), "workflow jobs")
    job = require_mapping(jobs.get(job_id), f"workflow job {job_id}")
    steps = job.get("steps")
    assert isinstance(steps, list), f"workflow job {job_id} must have steps"
    stage_index = next(
        index
        for index, raw_step in enumerate(steps)
        if require_mapping(raw_step, f"workflow job {job_id} step {index}").get("name")
        == "Stage artefacts"
    )
    steps.insert(
        stage_index + 1,
        {
            "name": "Capture staged paths",
            "id": "stage_paths",
            "shell": "bash",
            "run": run,
        },
    )


def _restore_mutated_step(
    workflow: dict[str, object], job_id: str, step_name: str, run: str
) -> None:
    """Restore one old run block, inserting the removed capture step if needed."""
    if step_name == "Capture staged paths":
        _restore_capture_step(workflow, job_id, run)
    else:
        _find_step(workflow, job_id, step_name)["run"] = run


def test_baseline_workflows_have_no_expression_source_in_run_blocks() -> None:
    """Accept both checked-in release workflows before mutation."""
    for workflow_name, workflow in _workflow_sources().items():
        assert not run_expression_violations(workflow_name, workflow), (
            f"{workflow_name} must not interpolate expressions into run blocks"
        )


def test_wix_and_draft_release_inputs_use_environment_variables() -> None:
    """Keep the release tag and event data out of Python source text."""
    workflows = _workflow_sources()
    wix = _find_step(
        workflows["release.yml"], "metadata", "Resolve WiX extension version"
    )
    wix_env = require_mapping(wix.get("env"), "WiX step environment")
    assert wix.get("run") == (
        "uv run --no-project --python 3.14 scripts/resolve_wix_extension_version.py"
    ), "the WiX step must delegate its logic to the tested Python script"
    assert wix_env == {
        "INPUT_EVENT_NAME": "${{ github.event_name }}",
        "INPUT_EVENT_PATH": "${{ github.event_path }}",
    }, "the WiX script inputs must be passed through the environment"

    draft = _find_step(
        workflows["release.yml"], "release", "Ensure release exists (draft)"
    )
    draft_env = require_mapping(draft.get("env"), "draft-release environment")
    assert draft_env.get("INPUT_TAG") == "${{ github.ref_name }}", (
        "the release tag must be an environment value"
    )
    assert draft_env.get("GITHUB_TOKEN") == "${{ secrets.GITHUB_TOKEN }}", (
        "the GitHub API token must reach gh through the process environment"
    )
    assert draft_env.get("GH_TOKEN") == "${{ secrets.GITHUB_TOKEN }}", (
        "gh must receive its token through the process environment"
    )


def test_upload_error_text_uses_a_quoted_environment_reference() -> None:
    """Keep shared-action error text out of shell source while printing it."""
    step = _find_step(
        _workflow_sources()["release.yml"], "release", "Check asset upload errors"
    )
    env = require_mapping(step.get("env"), "upload-error environment")
    assert env == {
        "ERROR_MESSAGE": "${{ steps.upload_assets.outputs.error-message }}"
    }, "shared-action error text must be passed through the environment"
    run = str(step.get("run", ""))
    assert "printf '%s\\n' \"$ERROR_MESSAGE\"" in run, (
        "the complete error text must be printed as one quoted value"
    )
    assert "exit 1" in run, "the error step must fail the workflow"


@pytest.mark.parametrize(("workflow_name", "job_id", "step_name", "run"), ORIGINAL_RUNS)
def test_each_historical_interpolation_is_rejected(
    workflow_name: str, job_id: str, step_name: str, run: str
) -> None:
    """Reject one restored unsafe step and identify its exact location."""
    workflow = copy.deepcopy(_workflow_sources()[workflow_name])
    _restore_mutated_step(workflow, job_id, step_name, run)
    jobs = require_mapping(workflow.get("jobs"), f"{workflow_name} jobs")
    job = require_mapping(jobs.get(job_id), f"{workflow_name} job {job_id}")
    steps = job.get("steps")
    assert isinstance(steps, list), "the mutated job must retain a steps list"
    index = next(
        position
        for position, raw_step in enumerate(steps)
        if require_mapping(
            raw_step, f"{workflow_name} job {job_id} step {position}"
        ).get("name")
        == step_name
    )

    expected_violation = (
        f"{workflow_name} job {job_id} step {index} ({step_name}): "
        "run block contains '${{'"
    )
    assert run_expression_violations(workflow_name, workflow) == [expected_violation], (
        "the historical expression must be the sole violation at its original step"
    )


def test_every_current_run_step_rejects_a_direct_expression() -> None:
    """Inject one expression at a time and require only that step to fail."""
    for workflow_name, baseline in _workflow_sources().items():
        for job_id, index, original_step in _step_contexts(workflow_name, baseline):
            run = original_step.get("run")
            if not isinstance(run, str):
                continue
            workflow = copy.deepcopy(baseline)
            jobs = require_mapping(workflow.get("jobs"), f"{workflow_name} jobs")
            job = require_mapping(jobs.get(job_id), f"{workflow_name} job {job_id}")
            steps = job.get("steps")
            assert isinstance(steps, list), "the current mutated job must retain steps"
            step = require_mapping(
                steps[index], f"{workflow_name} job {job_id} step {index}"
            )
            step["run"] = run + "\necho '${{ github.ref_name }}'"
            step_name = str(step.get("name", "<unnamed>"))
            expected_violation = (
                f"{workflow_name} job {job_id} step {index} ({step_name}): "
                "run block contains '${{'"
            )
            assert run_expression_violations(workflow_name, workflow) == [
                expected_violation
            ], "each run step must reject its own injected expression"
