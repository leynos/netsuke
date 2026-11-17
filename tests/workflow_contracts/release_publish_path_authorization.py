"""Prove the release workflow decides writes from the caller's own policy.

``publish-release`` holds ``contents: write``. The pinned
``determine-release-modes`` action answers ``should-publish: true`` for every
``push`` before it consults the reusable ``publish`` input, so the workflow's
own authorization step must sit between the resolver and every consumer of its
answer. These checks pin that wiring: the step's raw inputs, its position, the
gates it feeds, and the job-level dry-run veto.
"""

import shlex

import authorize_release_writes as policy
from release_publish_needs import job_names
from release_publish_path_artifacts import MISSING, FieldEvaluation, evaluate_field
from workflow_loading import job_steps, require_mapping, workflow_job

AUTHORIZATION_STEP = "release_authorization"
RESOLVER_STEP = "release_modes"
AUTHORIZATION_SCRIPT = "scripts/authorize_release_writes.py"
#: The pinned interpreter invocation that must lead the step's script. Held as
#: command words so the check compares the executed command, not a text match.
_UV_TOKENS = ("uv", "run", "--no-project", "--python", "3.14")
#: The raw inputs the policy decides over. Each is passed through ``env`` so no
#: caller-controlled value is interpolated into the script text.
EXPECTED_INPUTS = {
    "INPUT_PUBLISH": "${{ inputs.publish }}",
    "INPUT_DRY_RUN": "${{ inputs.dry-run }}",
    "INPUT_EVENT_NAME": "${{ github.event_name }}",
    "INPUT_REF": "${{ github.ref }}",
    "INPUT_WORKFLOW_REF": "${{ github.workflow_ref }}",
    "INPUT_REPOSITORY": "${{ github.repository }}",
    "INPUT_RESOLVER_SHOULD_PUBLISH": (
        "${{ steps.release_modes.outputs['should-publish'] }}"
    ),
    "INPUT_RESOLVER_DRY_RUN": "${{ steps.release_modes.outputs['dry-run'] }}",
    "INPUT_RESOLVER_SHOULD_UPLOAD_WORKFLOW_ARTIFACTS": (
        "${{ steps.release_modes.outputs['should-upload-workflow-artifacts'] }}"
    ),
}
#: A reusable push caller that asked for no publication. The resolver selects
#: publication anyway, so this is exactly the reported vulnerability: every gate
#: downstream of the resolver must close.
_DENIED_BUT_SELECTED = policy.ReleaseContext(
    publish="false",
    dry_run="false",
    event_name="push",
    ref="refs/tags/v1.2.3",
    workflow_ref=(
        "leynos/netsuke/.github/workflows/release-dry-run.yml@refs/heads/main"
    ),
    repository="leynos/netsuke",
    resolver_should_publish="true",
    resolver_dry_run="false",
    resolver_should_upload_workflow_artifacts="true",
)


def _exact_index(
    steps: list[dict[str, object]],
    step_id: str,
    label: str,
    violations: list[str],
) -> int | None:
    """Return the index of the one step whose id matches exactly.

    Substring matching would let a decoy such as
    ``release_authorization_unused`` stand in for the real step in an ordering
    or gate check, so each required id must occur once and only once.

    Returns
    -------
    int | None
        The zero-based index, or ``None`` when the report was made.
    """
    matches = [index for index, step in enumerate(steps) if step.get("id") == step_id]
    if len(matches) != 1:
        violations.append(
            f"{label}: metadata must declare exactly one step with id "
            f"{step_id!r}, found {len(matches)}"
        )
        return None
    return matches[0]


def _authorization_step(
    release: dict[str, object], violations: list[str]
) -> dict[str, object] | None:
    """Return the metadata job's authorization step, or report it missing.

    The id must match exactly. GitHub binds a step's outputs to its own id, so a
    step renamed to ``release_authorization_unused`` would still satisfy a
    substring lookup while no consumer could read the outputs it writes.

    Returns
    -------
    dict[str, object] | None
        The uniquely identified step, or ``None`` when the report was made.
    """
    steps = job_steps(release, "metadata")
    index = _exact_index(steps, AUTHORIZATION_STEP, "authorization.step", violations)
    return None if index is None else steps[index]


def _check_condition(step: dict[str, object], violations: list[str]) -> None:
    """Require the decision to run for every invocation.

    The later checks read synthetic outputs, so a conditional step would let
    them confirm wiring for outputs a skipped step never writes. The policy is
    required exactly so that every run reaches a decision; an ``if`` can only
    narrow that.
    """
    if "if" in step:
        violations.append(
            "authorization.condition: the decision must run unconditionally, "
            f"found {step['if']!r}"
        )


def _command_tokens(script: str) -> list[str]:
    """Split one shell command into words, dropping a trailing comment.

    ``shlex`` in comment mode treats ``#`` at a word boundary as the start of a
    comment, so a script path that appears only in a comment is not mistaken for
    the command that runs the policy. Unbalanced quotes make ``shlex`` raise
    ``ValueError``; the caller reports that rather than treating the script as
    authorised.

    Returns
    -------
    list[str]
        The command words, with any comment removed.
    """
    return shlex.split(script, comments=True)


def _check_run(step: dict[str, object], violations: list[str]) -> None:
    """Require the step to run the policy script without interpolation."""
    run = step.get("run")
    if not isinstance(run, str):
        violations.append(f"authorization.run: step must run {AUTHORIZATION_SCRIPT}")
        return
    script = run.strip()
    if "\n" in script or "\r" in script:
        # A newline separates commands in the shell. Tokenising it away would
        # accept two commands whose words happen to concatenate to the expected
        # invocation, which the runner would fail to execute.
        violations.append(
            "authorization.run: the step must be one logical command, not "
            "several separated by a newline"
        )
        return
    try:
        tokens = _command_tokens(script)
    except ValueError as error:
        violations.append(f"authorization.run: unparseable command: {error}")
        return
    if tokens != [*_UV_TOKENS, AUTHORIZATION_SCRIPT]:
        # Comparing the whole command rejects every position but the executed
        # one: a trailing `# mention` is a comment, and a script passed after
        # `python -c` is an argument Python never runs.
        violations.append(
            "authorization.run: step must be exactly "
            f"{' '.join([*_UV_TOKENS, AUTHORIZATION_SCRIPT])!r} so the policy "
            "is the script that runs"
        )
    if "${{" in run:
        violations.append(
            "authorization.run: untrusted values must arrive through env, "
            "never interpolated into the script text"
        )


def _check_env(step: dict[str, object], violations: list[str]) -> None:
    """Require every policy input to arrive from the expected context."""
    env = require_mapping(step.get("env"), "authorization.env")
    for name, expected in EXPECTED_INPUTS.items():
        if name not in env:
            violations.append(f"authorization.env.{name}: input is missing")
            continue
        if env[name] != expected:
            violations.append(
                f"authorization.env.{name}: resolved to {env[name]!r}, "
                f"expected {expected!r}"
            )
    unexpected = sorted(
        str(name)
        for name in env
        if str(name).startswith("INPUT_") and name not in EXPECTED_INPUTS
    )
    if unexpected:
        violations.append(f"authorization.env: unexpected policy inputs {unexpected!r}")


def _gate_context() -> dict[str, object]:
    """Build the step outputs of a denied run the resolver wanted to publish.

    Returns
    -------
    dict[str, object]
        Expression contexts for the metadata job, which declares no
        dependencies, so the implicit job-level success check is vacuous.
    """
    decision = policy.authorize_release_writes(_DENIED_BUT_SELECTED)
    return {
        "needs": {},
        "steps": {
            RESOLVER_STEP: {"outputs": dict(_resolver_outputs())},
            AUTHORIZATION_STEP: {"outputs": policy.render_outputs(decision)},
        },
    }


def _resolver_outputs() -> dict[str, str]:
    """Return the resolver's answer for the denied run's push trigger."""
    return {
        "should-publish": _DENIED_BUT_SELECTED.resolver_should_publish,
        "dry-run": _DENIED_BUT_SELECTED.resolver_dry_run,
        "should-upload-workflow-artifacts": (
            _DENIED_BUT_SELECTED.resolver_should_upload_workflow_artifacts
        ),
    }


def _check_step_order(release: dict[str, object], violations: list[str]) -> None:
    """Require the decision to sit after the resolver and before its gates."""
    steps = job_steps(release, "metadata")
    order: list[int] = []
    for step_id in (
        RESOLVER_STEP,
        AUTHORIZATION_STEP,
        "validate_publish_ref",
        "ensure_version",
    ):
        index = _exact_index(steps, step_id, "authorization.order", violations)
        if index is None:
            return
        order.append(index)
    if order != sorted(order):
        violations.append(
            "authorization.order: the decision must follow the resolver and "
            "precede the publishing gates"
        )


def _is_enabled(value: object) -> bool:
    """Report whether a workflow field switches its gated behaviour on.

    GitHub Actions passes ``with:`` values to an action as strings, so a
    ``check-tag`` that resolves to ``"true"`` enables version checking exactly
    as the boolean ``true`` does. Comparing against ``True`` alone would accept
    the string and leave the gate open for a denied run.

    Returns
    -------
    bool
        Whether the field switches the gated behaviour on.
    """
    if isinstance(value, bool):
        return value
    return isinstance(value, str) and value.strip().lower() == "true"


def _check_gates(release: dict[str, object], violations: list[str]) -> None:
    """Require the publishing gates to close on a denied-but-selected run."""
    steps = job_steps(release, "metadata")
    context = _gate_context()
    validate_index = _exact_index(
        steps, "validate_publish_ref", "authorization.gates", violations
    )
    version_index = _exact_index(
        steps, "ensure_version", "authorization.gates", violations
    )
    if validate_index is None or version_index is None:
        return
    validate = steps[validate_index]
    version = steps[version_index]
    gate = evaluate_field(
        validate.get("if", True),
        FieldEvaluation(
            contexts=context,
            violations=violations,
            label="authorization.validate-ref-gate",
            job_level=True,
        ),
    )
    if gate is MISSING or bool(gate):
        violations.append(
            "authorization.validate-ref-gate: the ref gate stayed open for a "
            "denied reusable push"
        )
    with_values = require_mapping(version.get("with"), "ensure_version.with")
    check_tag = evaluate_field(
        with_values.get("check-tag", MISSING),
        FieldEvaluation(
            contexts=context,
            violations=violations,
            label="authorization.version-tag-gate",
        ),
    )
    if check_tag is MISSING or _is_enabled(check_tag):
        violations.append(
            "authorization.version-tag-gate: version checking stayed enabled for "
            "a denied reusable push"
        )


def _check_publish_job_veto(release: dict[str, object], violations: list[str]) -> None:
    """Require the write job to refuse a run whose metadata says dry run.

    The policy never emits ``should_publish: true`` beside ``dry_run: true``, so
    this pair is unreachable from the authorization step. That unreachability is
    what makes the context decisive: with ``should_publish`` already true, the
    job stays closed only through its own dry-run clause, so a guard whose veto
    is missing, inverted, or vacuous is reported here rather than hiding behind
    a false ``should_publish``. The structural check below keeps the output
    named in the guard, and the publication scenarios prove the guard still
    opens for a real publishing run.
    """
    job = workflow_job(release, "publish-release")
    guard = job.get("if", True)
    if "needs.metadata.outputs.dry_run" not in str(guard):
        violations.append(
            "publish-release.dry-run-veto: the write job guard must name "
            "needs.metadata.outputs.dry_run"
        )
    outputs = {
        "should_publish": "true",
        "dry_run": "true",
        "should_upload_workflow_artifacts": "false",
        "should_upload_package_artifacts": "false",
    }
    needs: dict[str, object] = {}
    for name in job_names(job.get("needs", [])) or []:
        dependency: dict[str, object] = {"result": "success"}
        if name == "metadata":
            dependency["outputs"] = outputs
        needs[name] = dependency
    value = evaluate_field(
        guard,
        FieldEvaluation(
            contexts={"needs": needs, "status": {"cancelled": False}},
            violations=violations,
            label="publish-release.dry-run-veto",
            job_level=True,
        ),
    )
    if value is MISSING or bool(value):
        violations.append(
            "publish-release.dry-run-veto: the write job must not run while the "
            "release is in dry-run mode"
        )


def check_authorization_wiring(
    release: dict[str, object], violations: list[str]
) -> None:
    """Check the authorization step, the gates it feeds, and the job veto."""
    step = _authorization_step(release, violations)
    if step is not None:
        _check_condition(step, violations)
        _check_run(step, violations)
        _check_env(step, violations)
        _check_step_order(release, violations)
        _check_gates(release, violations)
    _check_publish_job_veto(release, violations)
