"""Adversarially exercise the isolated-secret-job invariant.

``trust_boundary_test.py`` holds the workflow files against the boundary;
these cases hold the boundary helper itself, by feeding it jobs and steps
that place the CodeScene credential. Every mutated carrier must be rejected,
so a widened helper cannot quietly accept a secret reference the trusted
workflow contract forbids.
"""

from trust_boundary_invariants import (
    CREDENTIAL_ENVIRONMENT_KEY,
    INDEXED_SECRET_EXPRESSIONS,
    REQUIRED_SECRET_JOB_PERMISSIONS,
    SECRET_EXPRESSION,
    TOKEN_PRESENCE_GUARD,
    is_isolated_secret_job,
)


def test_isolated_secret_job_detects_non_env_secret_references() -> None:
    """Reject secret references placed outside a step's environment mapping."""
    isolated: dict[str, object] = {"permissions": REQUIRED_SECRET_JOB_PERMISSIONS}
    guarded_secret_step: dict[str, object] = {
        "name": "Submit",
        "if": TOKEN_PRESENCE_GUARD,
        "env": {CREDENTIAL_ENVIRONMENT_KEY: "${{ secrets.CS_ACCESS_TOKEN }}"},
    }

    assert is_isolated_secret_job(isolated, [guarded_secret_step]), (
        "the guarded step-local carrier must satisfy the secret-job boundary"
    )

    mutations: list[dict[str, object]] = [
        {
            "name": "Submit",
            "if": TOKEN_PRESENCE_GUARD,
            "run": "echo ${{ secrets.CS_ACCESS_TOKEN }}",
        },
        {
            "name": "Submit",
            "if": TOKEN_PRESENCE_GUARD,
            "with": {"access-token": "${{ secrets.CS_ACCESS_TOKEN }}"},
        },
        {
            "name": "Submit",
            "if": TOKEN_PRESENCE_GUARD,
            "run": f"echo {INDEXED_SECRET_EXPRESSIONS[0]}",
        },
        {
            "name": "Submit",
            "if": TOKEN_PRESENCE_GUARD,
            "with": {"access-token": INDEXED_SECRET_EXPRESSIONS[0]},
        },
    ]
    for mutation in mutations:
        assert not is_isolated_secret_job(isolated, [guarded_secret_step, mutation]), (
            f"secret reference in {sorted(mutation)} must break isolation"
        )


def _guarded_secret_step() -> dict[str, object]:
    """Return one minimal step that carries the guarded exact credential."""
    return {
        "name": "Submit",
        "if": TOKEN_PRESENCE_GUARD,
        "env": {CREDENTIAL_ENVIRONMENT_KEY: SECRET_EXPRESSION},
    }


def test_isolated_secret_job_rejects_missing_credential_carrier() -> None:
    """Reject a secret job that has no credential-carrying step."""
    job: dict[str, object] = {"permissions": REQUIRED_SECRET_JOB_PERMISSIONS}

    assert not is_isolated_secret_job(job, [{"name": "Submit"}]), (
        "a secret job without a credential carrier must be rejected"
    )


def test_isolated_secret_job_rejects_multiple_credential_carriers() -> None:
    """Reject credential placement in more than one step environment."""
    job: dict[str, object] = {"permissions": REQUIRED_SECRET_JOB_PERMISSIONS}
    first = _guarded_secret_step()
    second = _guarded_secret_step() | {"name": "Duplicate submit"}

    assert not is_isolated_secret_job(job, [first, second]), (
        "multiple credential carriers must be rejected"
    )


def test_isolated_secret_job_rejects_unguarded_credential_carrier() -> None:
    """Reject a credential carrier that lacks the token-presence guard."""
    job: dict[str, object] = {"permissions": REQUIRED_SECRET_JOB_PERMISSIONS}
    unguarded = _guarded_secret_step() | {"if": "always()"}

    assert not is_isolated_secret_job(job, [unguarded]), (
        "a carrier without the token-presence guard must be rejected"
    )


def test_isolated_secret_job_rejects_different_credential_expression() -> None:
    """Reject a credential carrier whose value differs from the secret expression."""
    job: dict[str, object] = {"permissions": REQUIRED_SECRET_JOB_PERMISSIONS}
    mismatched = _guarded_secret_step() | {
        "env": {CREDENTIAL_ENVIRONMENT_KEY: "${{ secrets.OTHER_TOKEN }}"}
    }

    assert not is_isolated_secret_job(job, [mismatched]), (
        "a carrier with another secret expression must be rejected"
    )
