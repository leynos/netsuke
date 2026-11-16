"""Hold the consumer smoke to holding no credential of any kind.

The smoke reads public bytes and runs one already-built executable, so a token
in its environment could only widen the blast radius of a compromised smoke.
Three separate scopes can define one: the workflow's top-level ``env``, which
reaches every job; the smoke job and its steps, whose environment the job-level
scan reads; and the composite action the job calls, whose steps run in the same
process but appear nowhere in the workflow. All three are read here, so an edit
that reaches for a token fails a contract wherever it is made. A step can also
expand ``${{ github.token }}`` in its shell without declaring any environment,
so every step's executable lines are scanned alongside its mappings.

The rest of the smoke's shape lives in ``release_publish_path_consumer_shape``,
which owns the low-level readers these checks share and the constants that name
the documents. The scenario composition is in
``release_publish_path_consumer_smoke``.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_publish_path_consumer_shape import (
    CONSUMER_SMOKE_ACTION_PATH,
    CONSUMER_SMOKE_JOB,
    CONSUMER_SMOKE_PERMISSIONS,
    TOKEN_EXPRESSION,
    TOKEN_NAMES,
    action_steps,
    command_lines,
    steps_of,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def _environments(
    release: dict[str, object], job: dict[str, object]
) -> cabc.Iterator[tuple[str, dict[str, object]]]:
    """Yield every environment mapping that can reach the smoke, with its scope.

    Yields
    ------
    tuple[str, dict[str, object]]
        Each scope's label and its environment, in the order a variable
        reaches the child: the workflow's top level, then the job, then its
        steps. Scopes that declare no environment are skipped.
    """
    workflow_env = release.get("env")
    if isinstance(workflow_env, dict):
        yield "the workflow environment", workflow_env
    job_env = job.get("env")
    if isinstance(job_env, dict):
        yield "the job environment", job_env
    for index, step in enumerate(steps_of(job)):
        step_env = step.get("env")
        if isinstance(step_env, dict):
            yield f"step {step.get('name', index)}", step_env


def _leaked_credentials(scope: cabc.Mapping[str, object]) -> list[str]:
    """Return the names in one environment that hold or resolve to a credential.

    Returns
    -------
    list[str]
        The offending variable names, sorted so a violation reads the same way
        however the environment was ordered.
    """
    return sorted(
        str(name)
        for name, value in scope.items()
        if str(name).upper() in TOKEN_NAMES or TOKEN_EXPRESSION.search(str(value))
    )


def _credential_lines(run: object) -> list[str]:
    """Return the executable lines of a step that name a credential.

    A step needs no environment to expand ``${{ github.token }}``: the
    expression in its script reaches the same credential, so a scan of mappings
    alone would miss it.

    Returns
    -------
    list[str]
        The offending command lines, so a violation quotes the command rather
        than the whole script.
    """
    return [line for line in command_lines(run) if TOKEN_EXPRESSION.search(line)]


def check_smoke_privileges(
    release: dict[str, object], job: dict[str, object], violations: list[str]
) -> None:
    """Require read-only contents and no token, environment, or OIDC grant.

    Notes
    -----
    The workflow's top level is scanned as well as the job, because a variable
    declared there reaches every job without appearing in any of them. The
    composite action's steps are a separate scan, in
    :func:`check_smoke_action_environment`.
    """
    if job.get("permissions") != CONSUMER_SMOKE_PERMISSIONS:
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.permissions: expected contents: read and "
            "nothing else"
        )
    if "environment" in job:
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.environment: the smoke must not require an "
            "environment's secrets or protection rules"
        )
    for label, scope in _environments(release, job):
        leaked = _leaked_credentials(scope)
        if leaked:
            violations.append(
                f"{CONSUMER_SMOKE_JOB}.token: {label} must not define a "
                f"credential, got {leaked}"
            )
    for index, step in enumerate(steps_of(job)):
        leaked_lines = _credential_lines(step.get("run"))
        if leaked_lines:
            violations.append(
                f"{CONSUMER_SMOKE_JOB}.token: step {step.get('name', index)} "
                f"must not read a credential expression, got {leaked_lines[0]!r}"
            )


def check_smoke_action_environment(
    action: dict[str, object] | None, violations: list[str]
) -> None:
    """Require no step of the action to define a credential.

    The job-level scan cannot see these steps: the composite action's
    environment is declared in its own manifest, and every download and the
    probe itself run inside it. A token added there would reach the same
    process while the job's environment stayed clean, so the action is exactly
    where a credential would otherwise hide.
    """
    if action is None:
        # A missing manifest is reported once, by the declared-inputs check;
        # repeating it here would only bury the cause under its consequences.
        return
    for index, step in enumerate(action_steps(action)):
        label = f"{CONSUMER_SMOKE_ACTION_PATH} step {step.get('name', index)}"
        scope = step.get("env")
        leaked = _leaked_credentials(scope) if isinstance(scope, dict) else []
        if leaked:
            violations.append(
                f"{CONSUMER_SMOKE_JOB}.token: {label} must not define a "
                f"credential, got {leaked}"
            )
        leaked_lines = _credential_lines(step.get("run"))
        if leaked_lines:
            violations.append(
                f"{CONSUMER_SMOKE_JOB}.token: {label} must not read a credential "
                f"expression, got {leaked_lines[0]!r}"
            )


__all__ = [
    "check_smoke_action_environment",
    "check_smoke_privileges",
]
