"""Define pure validators for the PR coverage trust boundary.

The trusted coverage workflow receives an artefact produced by code that a
pull request controls. These helpers do not parse YAML or touch the filesystem:
checked-in workflow tests and bounded property tests supply normalized mappings
so the same rules reject secret leakage, missing guards, and PR checkouts.
"""

import collections.abc as cabc

CREDENTIAL_ENVIRONMENT_KEY = "CS_ACCESS_TOKEN"
TOKEN_PRESENCE_GUARD = f"env.{CREDENTIAL_ENVIRONMENT_KEY} != ''"
SECRET_EXPRESSION = f"${{{{ secrets.{CREDENTIAL_ENVIRONMENT_KEY} }}}}"
INDEXED_SECRET_EXPRESSIONS = (
    f"${{{{ secrets['{CREDENTIAL_ENVIRONMENT_KEY}'] }}}}",
    f'${{{{ secrets["{CREDENTIAL_ENVIRONMENT_KEY}"] }}}}',
)
SECRET_EXPRESSIONS = (SECRET_EXPRESSION, *INDEXED_SECRET_EXPRESSIONS)
TRUSTED_CHECKOUT_REF = "${{ github.event.repository.default_branch }}"
REQUIRED_SECRET_JOB_PERMISSIONS = {
    "actions": "read",
    "checks": "write",
    "contents": "read",
}


def contains_text(value: object, needle: str) -> bool:
    """Return whether a nested workflow value contains an exact text fragment.

    Parameters
    ----------
    value
        A parsed workflow value: mapping, sequence, string, or any other leaf
        treated as opaque.
    needle
        The exact text fragment to search for, such as a credential name.

    Returns
    -------
    bool
        Whether the fragment appears in the string or in any nested string
        reached through mappings (both keys and values) and sequences.

    Examples
    --------
    >>> contains_text({"with": {"token": "secrets.CS_ACCESS_TOKEN"}}, "CS_ACCESS_TOKEN")
    True
    >>> contains_text(["contents: read"], "CS_ACCESS_TOKEN")
    False
    """
    match value:
        case str():
            return needle in value
        case cabc.Mapping():
            return any(
                contains_text(item, needle) for entry in value.items() for item in entry
            )
        case cabc.Sequence():
            return any(contains_text(item, needle) for item in value)
        case _:
            return False


def job_references_secret(steps: list[dict[str, object]], name: str) -> bool:
    """Return whether any job step references the named secret or environment key.

    Parameters
    ----------
    steps
        The parsed steps of one workflow job.
    name
        The secret or environment key name to search for.

    Returns
    -------
    bool
        Whether any step mentions the name anywhere in its parsed mapping.

    Examples
    --------
    >>> job_references_secret(
    ...     [{"env": {"CS_ACCESS_TOKEN": "${{ secrets.CS_ACCESS_TOKEN }}"}}],
    ...     "CS_ACCESS_TOKEN",
    ... )
    True
    >>> job_references_secret([{"name": "Build"}], "CS_ACCESS_TOKEN")
    False
    """
    return any(contains_text(step, name) for step in steps)


def is_isolated_secret_job(
    job: dict[str, object], steps: list[dict[str, object]]
) -> bool:
    """Return whether a secret-bearing job has the required local boundary.

    A job passes when it carries no job-level environment mapping, keeps the
    exact least-privilege permission set, exposes the credential in exactly
    one step environment, and that step alone carries the token presence
    guard. No step may name the credential in ``run`` or ``with``, and no
    step may check out anything other than the trusted default-branch
    reference.

    Returns
    -------
    bool
        Whether the job satisfies every trust-boundary invariant.
    """
    if job.get("env"):
        return False
    if job.get("permissions") != REQUIRED_SECRET_JOB_PERMISSIONS:
        return False
    if _isolated_secret_step(steps) is None:
        return False
    if any(_references_secret_in_executable(step) for step in steps):
        return False
    has_untrusted_checkout = any(_checks_out_untrusted_ref(step) for step in steps)
    return not has_untrusted_checkout


def _isolated_secret_step(
    steps: list[dict[str, object]],
) -> dict[str, object] | None:
    """Return the sole guarded step that carries the exact credential value."""
    credential_steps = [
        step
        for step in steps
        if contains_text(step.get("env", {}), CREDENTIAL_ENVIRONMENT_KEY)
    ]
    if len(credential_steps) != 1:
        return None
    credential_step = credential_steps[0]
    if credential_step.get("if") != TOKEN_PRESENCE_GUARD:
        return None
    if not _carries_step_local_secret_expression(credential_step):
        return None
    return credential_step


def _carries_step_local_secret_expression(step: dict[str, object]) -> bool:
    """Return whether a step environment maps the credential from its secret."""
    environment = step.get("env")
    return (
        isinstance(environment, cabc.Mapping)
        and environment.get(CREDENTIAL_ENVIRONMENT_KEY) == SECRET_EXPRESSION
    )


def _references_secret_in_executable(step: dict[str, object]) -> bool:
    """Return whether a step consumes the credential secret in ``run`` or ``with``.

    Environment mappings are excluded: the guarded carrier step and any
    ``env.CS_ACCESS_TOKEN`` consumer expressions are the sanctioned local
    pattern, so only raw dotted or indexed ``secrets`` expressions reaching
    shell commands or action inputs count as exfiltration routes.

    Returns
    -------
    bool
        Whether ``run`` or ``with`` mentions a raw secret expression.
    """
    executable_surfaces = (
        step.get("run", ""),
        step.get("with", {}),
    )
    return any(
        contains_text(surface, expression)
        for surface in executable_surfaces
        for expression in SECRET_EXPRESSIONS
    )


def _checks_out_untrusted_ref(step: dict[str, object]) -> bool:
    """Return whether a checkout step lacks the explicit trusted reference."""
    if "actions/checkout@" not in str(step.get("uses", "")):
        return False
    with_ = step.get("with")
    return not isinstance(with_, cabc.Mapping) or (
        with_.get("ref") != TRUSTED_CHECKOUT_REF
    )
