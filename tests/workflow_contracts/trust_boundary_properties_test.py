"""Generate mutations that must not weaken the coverage trust boundary.

The safe baseline has a step-local CodeScene secret, an explicit empty-secret
guard, and a default-branch checkout. Bounded mutations model the common
regressions: adding the secret to untrusted PR execution, removing the guard,
or checking out the PR revision in the trusted job.
"""

from hypothesis import example, given, settings
from hypothesis import strategies as st
from trust_boundary_invariants import (
    CREDENTIAL_ENVIRONMENT_KEY,
    INDEXED_SECRET_EXPRESSIONS,
    REQUIRED_SECRET_JOB_PERMISSIONS,
    TOKEN_PRESENCE_GUARD,
    TRUSTED_CHECKOUT_REF,
    is_isolated_secret_job,
    job_references_secret,
)

MUTATIONS = (
    "valid",
    "secret-in-pull-request-job",
    "secret-missing-if-guard",
    "checkout-untrusted-ref",
    "indexed-secret-in-run",
    "indexed-secret-in-with",
)


def _safe_job() -> tuple[dict[str, object], list[dict[str, object]]]:
    """Build the minimum trusted job and its isolated secret step."""
    return (
        {
            "runs-on": "namespace-profile-netsuke",
            "permissions": REQUIRED_SECRET_JOB_PERMISSIONS,
        },
        [
            {
                "name": "Check out trusted validation tooling",
                "uses": "actions/checkout@pinned",
                "with": {"ref": TRUSTED_CHECKOUT_REF},
            },
            {
                "name": "Check coverage against CodeScene gates",
                "if": TOKEN_PRESENCE_GUARD,
                "env": {CREDENTIAL_ENVIRONMENT_KEY: "${{ secrets.CS_ACCESS_TOKEN }}"},
            },
        ],
    )


def _mutated_boundary(
    mutation: str,
) -> tuple[dict[str, object], list[dict[str, object]]]:
    """Apply one required trust-boundary regression to a safe baseline."""
    job, steps = _safe_job()
    if mutation == "secret-in-pull-request-job":
        return job, [
            {
                "name": "Run PR code",
                "env": {CREDENTIAL_ENVIRONMENT_KEY: "credential"},
            }
        ]
    if mutation == "secret-missing-if-guard":
        steps[1].pop("if")
    if mutation == "checkout-untrusted-ref":
        steps[0]["with"] = {"ref": "${{ github.event.workflow_run.head_sha }}"}
    if mutation == "indexed-secret-in-run":
        steps.append({"name": "Leak", "run": f"echo {INDEXED_SECRET_EXPRESSIONS[0]}"})
    if mutation == "indexed-secret-in-with":
        steps.append({
            "name": "Leak",
            "uses": "example/action@pinned",
            "with": {"token": INDEXED_SECRET_EXPRESSIONS[0]},
        })
    return job, steps


@settings(max_examples=24, derandomize=True, deadline=None)
@example(mutation="secret-in-pull-request-job")
@example(mutation="secret-missing-if-guard")
@example(mutation="checkout-untrusted-ref")
@example(mutation="indexed-secret-in-run")
@example(mutation="indexed-secret-in-with")
@given(mutation=st.sampled_from(MUTATIONS))
def test_generated_boundary_mutations_accept_only_safe_configuration(
    mutation: str,
) -> None:
    """Accept the safe baseline and reject every required hostile mutation."""
    job, steps = _mutated_boundary(mutation)
    if mutation == "secret-in-pull-request-job":
        assert job_references_secret(steps, CREDENTIAL_ENVIRONMENT_KEY), (
            "the mutation must model a credential in untrusted PR execution"
        )
        return
    assert is_isolated_secret_job(job, steps) is (mutation == "valid"), (
        f"mutation={mutation!r} must be rejected unless it is the safe baseline"
    )
