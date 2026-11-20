"""Enumerate every release invocation the publish-path contract evaluates.

The value objects, the release-mode constants, and the resolver the policy reads
live in ``release_publish_path_scenario_model``, so this module holds only the
table: one row per invocation shape and dependency outcome the workflow must
handle.
"""

from release_publish_path_scenario_model import (
    BRANCH_REF,
    CALLER_WORKFLOW_REF,
    CRATES_IO_NEEDS,
    DIRECT_WORKFLOW_REF,
    GITHUB_SHA,
    PUBLISHING_DRY_RUN,
    PULL_REF,
    TAG_REF,
    VERIFY_JOB,
    Invocation,
    Scenario,
    needs_results,
)


def _prerequisite_scenarios(outcome: str) -> tuple[Scenario, ...]:
    """Create one publishing scenario for each failed or skipped prerequisite."""
    return tuple(
        Scenario(
            f"publish-{need}-{outcome}",
            "publish",
            "",
            needs_results(outcomes={need: outcome}),
            cancelled=False,
            release_runs=False,
        )
        for need in sorted(CRATES_IO_NEEDS)
    )


def _reusable_push(name: str, invocation: Invocation) -> Scenario:
    """Create a reusable invocation that inherited a push trigger."""
    return Scenario(
        name,
        "publish",
        "",
        needs_results(),
        cancelled=False,
        release_runs=False,
        invocation=invocation,
    )


SCENARIOS = (
    Scenario(
        "opened",
        "dry-run",
        "opened",
        needs_results(smoke="skipped"),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        "synchronize",
        "dry-run",
        "synchronize",
        needs_results(smoke="skipped"),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        "reopened",
        "dry-run",
        "reopened",
        needs_results(smoke="skipped"),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        "ready-for-review",
        "dry-run",
        "ready_for_review",
        needs_results(),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        # The realistic dry run: neither publisher runs, so the verification
        # job must be skipped even though the release staging job runs.
        "dry-run-publishers-skipped",
        "dry-run",
        "opened",
        needs_results(
            smoke="skipped",
            outcomes={"publish-crates-io": "skipped", "publish-release": "skipped"},
        ),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        "dry-run-failed-build",
        "dry-run",
        "opened",
        needs_results(smoke="skipped", outcomes={"build-linux": "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "tag-publish",
        "publish",
        "",
        needs_results(),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "tag-publish-skipped-smoke",
        "publish",
        "",
        needs_results(smoke="skipped"),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "tag-publish-failed-smoke",
        "publish",
        "",
        needs_results(smoke="failure"),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "tag-publish-failed-build",
        "publish",
        "",
        needs_results(outcomes={"build-linux": "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    _reusable_push(
        "reusable-push-publish-false",
        Invocation("false", "false", "push", TAG_REF, CALLER_WORKFLOW_REF),
    ),
    _reusable_push(
        "reusable-push-omitted-inputs",
        Invocation("", "", "push", TAG_REF, CALLER_WORKFLOW_REF),
    ),
    _reusable_push(
        "reusable-push-publish-true",
        Invocation("true", "false", "push", TAG_REF, CALLER_WORKFLOW_REF),
    ),
    Scenario(
        "reusable-push-dry-run-overrides-publish",
        "publish",
        "",
        needs_results(),
        cancelled=False,
        release_runs=True,
        invocation=Invocation("true", "true", "push", TAG_REF, CALLER_WORKFLOW_REF),
    ),
    _reusable_push(
        "reusable-branch-push-publish-true",
        Invocation("true", "false", "push", BRANCH_REF, CALLER_WORKFLOW_REF),
    ),
    Scenario(
        "direct-trigger-dry-run-input",
        "publish",
        "",
        needs_results(),
        cancelled=False,
        release_runs=True,
        invocation=Invocation("", "true", "push", TAG_REF, DIRECT_WORKFLOW_REF),
    ),
    Scenario(
        "pull-request-publish-true",
        "dry-run",
        "opened",
        needs_results(),
        cancelled=False,
        release_runs=True,
        invocation=Invocation(
            "true", "false", "pull_request", PULL_REF, CALLER_WORKFLOW_REF
        ),
    ),
    Scenario(
        "direct-push-resolver-dry-run",
        "publish",
        "",
        needs_results(),
        cancelled=False,
        release_runs=True,
        resolver=PUBLISHING_DRY_RUN,
    ),
    Scenario(
        # The gate ran and succeeded -- as an observation. Report-only
        # admission cannot satisfy a real publishing gate.
        "tag-publish-report-only-admission",
        "publish",
        "",
        needs_results(),
        cancelled=False,
        release_runs=False,
        admission_outputs={
            "enforcement": "false",
            "gate_outcome": "success",
            "candidate_commit": GITHUB_SHA,
        },
    ),
    Scenario(
        "tag-publish-failed-admission-gate",
        "publish",
        "",
        needs_results(),
        cancelled=False,
        release_runs=False,
        admission_outputs={
            "enforcement": "true",
            "gate_outcome": "failure",
            "candidate_commit": GITHUB_SHA,
        },
    ),
    Scenario(
        "tag-publish-mismatched-admission-candidate",
        "publish",
        "",
        needs_results(),
        cancelled=False,
        release_runs=False,
        admission_outputs={
            "enforcement": "true",
            "gate_outcome": "success",
            "candidate_commit": "b" * 40,
        },
    ),
    Scenario(
        "publish-crates-io-failure",
        "publish",
        "",
        needs_results(outcomes={"publish-crates-io": "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "publish-crates-io-skipped",
        "publish",
        "",
        needs_results(outcomes={"publish-crates-io": "skipped"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "publish-release-failure",
        "publish",
        "",
        needs_results(outcomes={"publish-release": "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "publish-release-skipped",
        "publish",
        "",
        needs_results(outcomes={"publish-release": "skipped"}),
        cancelled=False,
        release_runs=False,
    ),
    # A release left as a draft is not publicly downloadable, so a smoke that
    # ran anyway would fail on a 404 rather than skip. These two cases are what
    # make the finalization prerequisite observable: without them a guard that
    # dropped the finalize result but kept the dependency would still agree
    # with every other scenario.
    Scenario(
        "finalize-failure",
        "publish",
        "",
        needs_results(outcomes={"finalize-github-release": "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "finalize-skipped",
        "publish",
        "",
        needs_results(outcomes={"finalize-github-release": "skipped"}),
        cancelled=False,
        release_runs=False,
    ),
    # Both publishers succeeded, so only the verifier's own outcome stands
    # between this run and an un-drafted release. The skip row is a modelling
    # device rather than a reachable run: finalization's guard must refuse a
    # verifier that reported anything but success, and declaring the skip is
    # what exercises that clause without a second prerequisite failure.
    Scenario(
        "verify-release-inventory-failure",
        "publish",
        "",
        needs_results(outcomes={VERIFY_JOB: "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "verify-release-inventory-skipped",
        "publish",
        "",
        needs_results(outcomes={VERIFY_JOB: "skipped"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "neither-mode",
        "neither",
        "",
        needs_results(),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "cancelled-ready-for-review",
        "dry-run",
        "ready_for_review",
        needs_results(),
        cancelled=True,
        release_runs=False,
    ),
    Scenario(
        "cancelled-publishing",
        "publish",
        "",
        needs_results(),
        cancelled=True,
        release_runs=False,
    ),
    *_prerequisite_scenarios("failure"),
    *_prerequisite_scenarios("skipped"),
    # A job-level concurrency rule can cancel one pending dependency while the
    # run continues. That arrives as an ordinary non-successful result, not as
    # a cancelled run, so it must close the publishers and their dependants.
    *_prerequisite_scenarios("cancelled"),
)
