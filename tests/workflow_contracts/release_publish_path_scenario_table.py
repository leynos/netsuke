"""List the release modes and dependency outcomes every contract check shares.

The table is separated from the scenario model in
``release_publish_path_scenarios`` so each module stays a readable size. The
model module owns the constants and helpers; this module owns the finite list of
runs those helpers describe, which the reachability, verification, and
finalization checks all iterate.
"""

from release_publish_path_scenarios import (
    GITHUB_SHA,
    VERIFY_JOB,
    Scenario,
    _needs_results,
    _prerequisite_scenarios,
)

SCENARIOS = (
    Scenario(
        "opened",
        "dry-run",
        "opened",
        _needs_results(smoke="skipped"),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        "synchronize",
        "dry-run",
        "synchronize",
        _needs_results(smoke="skipped"),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        "reopened",
        "dry-run",
        "reopened",
        _needs_results(smoke="skipped"),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        "ready-for-review",
        "dry-run",
        "ready_for_review",
        _needs_results(),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        # The realistic dry run: neither publisher runs, so the verification
        # job must be skipped even though the release staging job runs.
        "dry-run-publishers-skipped",
        "dry-run",
        "opened",
        _needs_results(
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
        _needs_results(smoke="skipped", outcomes={"build-linux": "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        # The rehearsal is independent of packaging and admission, so either
        # one failing must still leave the read-only lane reachable.
        "dry-run-failed-package-crate",
        "dry-run",
        "opened",
        _needs_results(smoke="skipped", outcomes={"package-crate": "failure"}),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        "dry-run-failed-admission",
        "dry-run",
        "opened",
        _needs_results(
            smoke="skipped", outcomes={"release-admission-canaries": "failure"}
        ),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        "tag-publish",
        "publish",
        "",
        _needs_results(),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "tag-publish-skipped-smoke",
        "publish",
        "",
        _needs_results(smoke="skipped"),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "tag-publish-failed-smoke",
        "publish",
        "",
        _needs_results(smoke="failure"),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "tag-publish-failed-build",
        "publish",
        "",
        _needs_results(outcomes={"build-linux": "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        # The gate ran and succeeded -- as an observation. Report-only
        # admission cannot satisfy a real publishing gate.
        "tag-publish-report-only-admission",
        "publish",
        "",
        _needs_results(),
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
        _needs_results(),
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
        _needs_results(),
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
        _needs_results(outcomes={"publish-crates-io": "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "publish-crates-io-skipped",
        "publish",
        "",
        _needs_results(outcomes={"publish-crates-io": "skipped"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "publish-release-failure",
        "publish",
        "",
        _needs_results(outcomes={"publish-release": "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "publish-release-skipped",
        "publish",
        "",
        _needs_results(outcomes={"publish-release": "skipped"}),
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
        _needs_results(outcomes={"finalize-github-release": "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "finalize-skipped",
        "publish",
        "",
        _needs_results(outcomes={"finalize-github-release": "skipped"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        # Both publishers succeeded, so only the verifier's own outcome stands
        # between this run and an un-drafted release.
        "verify-release-inventory-failure",
        "publish",
        "",
        _needs_results(outcomes={VERIFY_JOB: "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "verify-release-inventory-skipped",
        "publish",
        "",
        _needs_results(outcomes={VERIFY_JOB: "skipped"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "neither-mode",
        "neither",
        "",
        _needs_results(),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "cancelled-ready-for-review",
        "dry-run",
        "ready_for_review",
        _needs_results(),
        cancelled=True,
        release_runs=False,
    ),
    Scenario(
        "cancelled-publishing",
        "publish",
        "",
        _needs_results(),
        cancelled=True,
        release_runs=False,
    ),
    *_prerequisite_scenarios("failure"),
    *_prerequisite_scenarios("skipped"),
    *_prerequisite_scenarios("cancelled"),
)
