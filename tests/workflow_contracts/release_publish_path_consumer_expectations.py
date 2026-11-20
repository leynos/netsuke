"""State what each consumer-smoke mutation must break.

The mutations themselves are applied in
``release_publish_path_consumer_mutations``, which re-exports the table below
alongside its appliers. The two are kept apart because they answer different
questions: this module is the reviewed claim about which scenario and which
contract a regression must trip, and a mutation whose claim no longer matches a
real violation is a contract that has stopped guarding anything.

Run via ``make test-workflow-contracts``.
"""

from release_publish_path_consumer_smoke import CONSUMER_SMOKE_JOB

#: The scenario and contract each mutation must break. ``static`` names a
#: violation that does not depend on a release scenario; every other label
#: names the scenario whose guard must resolve to the wrong result.
CONSUMER_SMOKE_EXPECTED_VIOLATIONS = {
    "drop-consumer-smoke-finalize-prerequisite": (
        ("static", f"{CONSUMER_SMOKE_JOB}.needs"),
    ),
    "ignore-consumer-smoke-finalize-result": (
        ("finalize-failure", f"{CONSUMER_SMOKE_JOB}-job"),
    ),
    # A non-publishing run also skips finalization, whose result the guard
    # already requires, so no resolved scenario can observe this clause on its
    # own: the guard is pinned against a run where every other clause holds.
    "remove-consumer-smoke-publish-guard": (("static", f"{CONSUMER_SMOKE_JOB}.guard"),),
    "remove-consumer-smoke-cancelled-guard": (
        ("cancelled-publishing", f"{CONSUMER_SMOKE_JOB}-job"),
    ),
    "remove-consumer-smoke-permissions": (
        ("static", f"{CONSUMER_SMOKE_JOB}.permissions"),
    ),
    "grant-consumer-smoke-contents-write": (
        ("static", f"{CONSUMER_SMOKE_JOB}.permissions"),
    ),
    "add-consumer-smoke-id-token": (("static", f"{CONSUMER_SMOKE_JOB}.permissions"),),
    "add-consumer-smoke-environment": (
        ("static", f"{CONSUMER_SMOKE_JOB}.environment"),
    ),
    "add-consumer-smoke-token-environment": (
        ("static", f"{CONSUMER_SMOKE_JOB}.token"),
    ),
    # A variable declared at the workflow's top level reaches the smoke without
    # appearing in the job, so a job-only scan would leave it unguarded.
    "add-consumer-smoke-workflow-token": (("static", f"{CONSUMER_SMOKE_JOB}.token"),),
    # The action declares its own step environments, so a credential added
    # there never appears in the job's. The contract must still refuse it.
    "add-consumer-smoke-action-token-environment": (
        ("static", f"{CONSUMER_SMOKE_JOB}.token"),
    ),
    # A step condition can stop the smoke while the job guard, the
    # dependencies, and the inputs all still read as correct.
    "add-consumer-smoke-invocation-guard": (
        ("static", f"{CONSUMER_SMOKE_JOB}.action"),
    ),
    "persist-consumer-smoke-checkout-credentials": (
        ("static", f"{CONSUMER_SMOKE_JOB}.checkout"),
    ),
    "select-consumer-smoke-pattern-artifact": (
        ("static", f"{CONSUMER_SMOKE_JOB}.artifacts"),
    ),
    # The name and the action inputs stay correct while the bytes land where
    # the verifier will never look for them.
    "redirect-consumer-smoke-artifact": (
        ("static", f"{CONSUMER_SMOKE_JOB}.artifacts"),
    ),
    # The remaining download still names its artefact exactly, so only a
    # contract that requires both to be present sees the missing evidence.
    "drop-consumer-smoke-artifact-download": (
        ("static", f"{CONSUMER_SMOKE_JOB}.artifacts"),
    ),
    # The download still names its artefact and its destination, so only a
    # contract that reads the step's condition notices it never runs.
    "disable-consumer-smoke-artifact-download": (
        ("static", f"{CONSUMER_SMOKE_JOB}.artifacts"),
    ),
    # The download runs, in the right place, but after the action has already
    # read the directory it fills, so only an ordering check sees it.
    "delay-consumer-smoke-artifact-download": (
        ("static", f"{CONSUMER_SMOKE_JOB}.artifacts"),
    ),
    # A smoke that cannot fail reports a release whose bytes were never
    # checked as verified, at the job level or on the one step that runs it.
    "absorb-consumer-smoke-job-failure": (
        ("static", f"{CONSUMER_SMOKE_JOB}.continue-on-error"),
    ),
    "absorb-consumer-smoke-step-failure": (
        ("static", f"{CONSUMER_SMOKE_JOB}.continue-on-error"),
    ),
    # A composite step that tolerates its own failure ends the action
    # successfully, so the job never sees the refusal.
    "absorb-consumer-smoke-action-failure": (
        ("static", f"{CONSUMER_SMOKE_JOB}.continue-on-error"),
    ),
    # The report step survives, so the run still reads as a complete smoke
    # while nothing verified the published bytes.
    "drop-consumer-smoke-verifier": (("static", f"{CONSUMER_SMOKE_JOB}.action"),),
    # A launcher that only prints the reviewed command runs nothing, so the
    # whole command line, not just its arguments, is part of the contract.
    "echo-consumer-smoke-verifier": (("static", f"{CONSUMER_SMOKE_JOB}.action"),),
    # An `exit 0` above the verifier leaves the reviewed command line intact
    # and unreachable, so the contract has to read the step's control flow and
    # not only the invocation it contains.
    "short-circuit-consumer-smoke-verifier": (
        ("static", f"{CONSUMER_SMOKE_JOB}.action"),
    ),
    # The invocation line is untouched, and `set +e` sits after the reviewed
    # fail-fast rather than above it, so only a contract that reads the
    # prologue and what follows it notices the refusal being absorbed.
    "hide-consumer-smoke-verifier-failure": (
        ("static", f"{CONSUMER_SMOKE_JOB}.action"),
    ),
    # The invocation stays on its own line with every reviewed argument; only
    # the branch around it makes it unreachable.
    "enclose-consumer-smoke-verifier-in-false-branch": (
        ("static", f"{CONSUMER_SMOKE_JOB}.action"),
    ),
    # `PLACEHOLDER=1 exit 0` has an assignment's spelling and bash's `exit 0`
    # semantics, so a reader that matches the line's shape rather than
    # tokenizing it waves the early exit through.
    "prefix-consumer-smoke-verifier-with-assignment-exit": (
        ("static", f"{CONSUMER_SMOKE_JOB}.action"),
    ),
    # `set -n` looks like the reviewed `set -euo pipefail` and makes bash read
    # the invocation without running it, so only a reader that checks which
    # options a prologue turns on refuses it.
    "disable-consumer-smoke-verifier-execution": (
        ("static", f"{CONSUMER_SMOKE_JOB}.action"),
    ),
    # Single quotes turn the evidence path into a literal string. The tokenized
    # command line is unchanged, so only a check that reads the line's quoting
    # notices that every lookup will fail at run time.
    "single-quote-consumer-smoke-verifier-reference": (
        ("static", f"{CONSUMER_SMOKE_JOB}.action"),
    ),
    # The assignment above the early exit legitimately spells the CLI's path,
    # so a reader that stops the prologue scan at the first line naming the CLI
    # reports the prologue as clean and lets the verifier be skipped.
    "mask-consumer-smoke-verifier-with-cli-named-assignment": (
        ("static", f"{CONSUMER_SMOKE_JOB}.action"),
    ),
    # The reviewed command line appears twice in one step, so a contract that
    # counts steps, or reads only the first invocation of each step, reports
    # one verified run while the action executes the verifier twice.
    "duplicate-consumer-smoke-verifier": (("static", f"{CONSUMER_SMOKE_JOB}.action"),),
    # A step declares no environment and still expands the workflow token in
    # its shell, so a scan of environment mappings alone would leave it.
    "read-consumer-smoke-token-in-run": (("static", f"{CONSUMER_SMOKE_JOB}.token"),),
    "read-consumer-smoke-action-token-in-run": (
        ("static", f"{CONSUMER_SMOKE_JOB}.token"),
    ),
    "compile-in-consumer-smoke-job": (("static", f"{CONSUMER_SMOKE_JOB}.compilation"),),
    "compile-in-consumer-smoke-action": (
        ("static", f"{CONSUMER_SMOKE_JOB}.compilation"),
    ),
}
MUTATION_LABELS = tuple(CONSUMER_SMOKE_EXPECTED_VIOLATIONS)

__all__ = [
    "CONSUMER_SMOKE_EXPECTED_VIOLATIONS",
    "MUTATION_LABELS",
]
