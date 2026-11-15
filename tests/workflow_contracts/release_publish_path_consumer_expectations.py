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
    "remove-consumer-smoke-publish-guard": (
        ("neither-mode", f"{CONSUMER_SMOKE_JOB}-job"),
    ),
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
