"""Resolve the metadata outputs a release run publishes to its consumers.

Every later job reads the mode from ``needs.metadata.outputs``, so a mis-wired
output is what turns an authorization decision into a release write. These
helpers evaluate the declarations against both step answers the metadata job may
read, which lets the contract report the scenarios an unchanged workflow would
misjudge.
"""

import typing as typ

import authorize_release_writes as adapter
from release_publish_path_artifacts import MISSING, FieldEvaluation, evaluate_field
from release_publish_path_authorization import AUTHORIZATION_STEP, RESOLVER_STEP
from workflow_loading import REPO_ROOT, require_mapping, workflow_job

if typ.TYPE_CHECKING:
    from release_publish_path_scenario_model import Scenario


def _step_contexts(scenario: Scenario) -> dict[str, object]:
    """Build both step answers the metadata job may read.

    The resolver's answer and the authorization decision are both present so an
    unchanged workflow resolves the resolver's answer and the contract reports
    the scenarios it would misjudge.

    Returns
    -------
    dict[str, object]
        The ``steps`` context both metadata-output sources resolve against.
    """
    return {
        "steps": {
            RESOLVER_STEP: {"outputs": dict(scenario.resolver_outputs)},
            AUTHORIZATION_STEP: {
                "outputs": adapter.render_outputs(scenario.authorization)
            },
        }
    }


def resolved_repo_name(release: dict[str, object], violations: list[str]) -> str:
    """Resolve the metadata ``repo_name`` output, which no mode changes.

    The artefact-name and download-pattern checks compare against the value this
    function returns, so a declaration that resolves to some other name would
    agree with itself and pass while publishing artefacts under a name no job
    can find. The resolved name is therefore checked against this repository,
    and the value is still returned so the later checks report it in context.

    Returns
    -------
    str
        The repository name the release workflow will use for artefacts.
    """
    declarations = require_mapping(
        workflow_job(release, "metadata").get("outputs"), "metadata.outputs"
    )
    expression = declarations.get("repo_name", MISSING)
    label = "metadata.repo_name"
    if expression is MISSING:
        violations.append(f"{label}: output is missing")
        return REPO_ROOT.name
    value = evaluate_field(
        expression,
        FieldEvaluation(
            contexts={"steps": {"repo_name": {"outputs": {"value": REPO_ROOT.name}}}},
            violations=violations,
            label=label,
        ),
    )
    if value is MISSING:
        # The evaluator has already recorded why the expression was refused.
        return REPO_ROOT.name
    if not isinstance(value, str):
        violations.append(f"{label}: resolved to {value!r}, expected a string")
        return REPO_ROOT.name
    if value != REPO_ROOT.name:
        violations.append(
            f"{label}: resolved to {value!r}, expected {REPO_ROOT.name!r}"
        )
    return value


def metadata_outputs(
    release: dict[str, object], scenario: Scenario, violations: list[str]
) -> dict[str, str]:
    """Resolve the release-mode outputs declared by the metadata job."""
    declarations = require_mapping(
        workflow_job(release, "metadata").get("outputs"), "metadata.outputs"
    )
    contexts = _step_contexts(scenario)
    outputs: dict[str, str] = {}
    for name, expected_value in scenario.outputs.items():
        expression = declarations.get(name, MISSING)
        label = f"{scenario.name}.metadata.{name}"
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
