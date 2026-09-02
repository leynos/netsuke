"""Define pure validators for the PR coverage trust boundary.

The trusted coverage workflow receives an artefact produced by code that a
pull request controls. These helpers do not parse YAML or touch the filesystem:
checked-in workflow tests and bounded property tests supply normalized mappings
so the same rules reject secret leakage, missing guards, and PR checkouts.
"""

import collections.abc as cabc

CREDENTIAL_ENVIRONMENT_KEY = "CS_ACCESS_TOKEN"
TOKEN_PRESENCE_GUARD = f"env.{CREDENTIAL_ENVIRONMENT_KEY} != ''"
TRUSTED_CHECKOUT_REF = "${{ github.event.repository.default_branch }}"
REQUIRED_SECRET_JOB_PERMISSIONS = {
    "actions": "read",
    "checks": "write",
    "contents": "read",
}


def contains_text(value: object, needle: str) -> bool:
    """Return whether a nested workflow value contains an exact text fragment."""
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
    """Return whether any job step references the named secret or environment key."""
    return any(contains_text(step, name) for step in steps)


def is_isolated_secret_job(
    job: dict[str, object], steps: list[dict[str, object]]
) -> bool:
    """Return whether a secret-bearing job has the required local boundary."""
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
    return not any(_checks_out_untrusted_ref(step) for step in steps)


def _checks_out_untrusted_ref(step: dict[str, object]) -> bool:
    """Return whether a checkout step lacks the explicit trusted reference."""
    if "actions/checkout@" not in str(step.get("uses", "")):
        return False
    with_ = step.get("with")
    return not isinstance(with_, cabc.Mapping) or (
        with_.get("ref") != TRUSTED_CHECKOUT_REF
    )
