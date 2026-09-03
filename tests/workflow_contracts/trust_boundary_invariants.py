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
    if job.get("env") or job.get("permissions") != REQUIRED_SECRET_JOB_PERMISSIONS:
        return False
    secret_steps = [
        step
        for step in steps
        if contains_text(step.get("env", {}), CREDENTIAL_ENVIRONMENT_KEY)
    ]
    if len(secret_steps) != 1:
        return False
    secret_step = secret_steps[0]
    if secret_step.get("if") != TOKEN_PRESENCE_GUARD:
        return False
    if not _carries_step_local_secret_expression(secret_step):
        return False
    if any(_references_secret_in_executable(step) for step in steps):
        return False
    return not any(_checks_out_untrusted_ref(step) for step in steps)


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
    pattern, so only the raw ``secrets.CS_ACCESS_TOKEN`` expression reaching
    shell commands or action inputs counts as an exfiltration route.

    Returns
    -------
    bool
        Whether ``run`` or ``with`` mentions the raw secret expression.
    """
    executable_surfaces = (
        step.get("run", ""),
        step.get("with", {}),
    )
    return any(
        contains_text(surface, SECRET_EXPRESSION) for surface in executable_surfaces
    )


def _checks_out_untrusted_ref(step: dict[str, object]) -> bool:
    """Return whether a checkout step lacks the explicit trusted reference."""
    if "actions/checkout@" not in str(step.get("uses", "")):
        return False
    with_ = step.get("with")
    return not isinstance(with_, cabc.Mapping) or (
        with_.get("ref") != TRUSTED_CHECKOUT_REF
    )
