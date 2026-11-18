"""Resolve the release-mode outputs the metadata job publishes.

The metadata job is the only place a run decides whether it publishes, which
artifacts it uploads, and what repository name its artifact names carry. Its
declarations are evaluated against the mode's expected outputs, so a reworded
expression that resolves to the wrong value is reported rather than trusted.
The module is separate from ``release_publish_path_contract`` to keep both
readable.
"""

from release_publish_path_artifacts import MISSING, FieldEvaluation, evaluate_field
from release_publish_path_scenarios import MODE_OUTPUTS, PACKAGE_UPLOAD
from workflow_loading import REPO_ROOT, require_mapping, workflow_job


def metadata_outputs(
    release: dict[str, object], mode: str, violations: list[str]
) -> dict[str, str]:
    """Resolve the release-mode outputs declared by the metadata job.

    Returns
    -------
    dict[str, str]
        The outputs that resolved, keyed by their declared name. Outputs that
        did not resolve are absent and their failure is recorded in
        *violations*.
    """
    declarations = require_mapping(
        workflow_job(release, "metadata").get("outputs"), "metadata.outputs"
    )
    steps = {
        "release_modes": {"outputs": MODE_OUTPUTS[mode]},
        "repo_name": {"outputs": {"value": REPO_ROOT.name}},
    }
    contexts: dict[str, object] = {"steps": steps}
    outputs: dict[str, str] = {}
    expected = {
        "should_publish": MODE_OUTPUTS[mode]["should-publish"],
        "dry_run": MODE_OUTPUTS[mode]["dry-run"],
        "should_upload_workflow_artifacts": MODE_OUTPUTS[mode][
            "should-upload-workflow-artifacts"
        ],
        "should_upload_package_artifacts": str(PACKAGE_UPLOAD[mode]).lower(),
        "repo_name": REPO_ROOT.name,
    }
    for name, expected_value in expected.items():
        expression = declarations.get(name, MISSING)
        label = f"metadata.{name}"
        if expression is MISSING:
            violations.append(f"{label}: output is missing")
            continue
        value = evaluate_field(
            expression,
            FieldEvaluation(contexts=contexts, violations=violations, label=label),
        )
        if value is MISSING:
            continue
        rendered = str(value).lower() if isinstance(value, bool) else value
        if not isinstance(rendered, str):
            violations.append(f"{label}: output did not resolve to a string or boolean")
            continue
        outputs[name] = rendered
        if rendered != expected_value:
            violations.append(
                f"{label}: resolved to {rendered!r}, expected {expected_value!r}"
            )
    return outputs
