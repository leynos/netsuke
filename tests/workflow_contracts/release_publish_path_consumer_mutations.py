"""Mutate the consumer smoke to prove the release-path contract rejects it.

Each entry names one plausible regression in the smoke's privilege, wiring, or
reachability, applies it to a deep copy, and states the scenario and contract
that must report it. A mutation whose expectation no longer matches a real
violation is a contract that has stopped guarding anything, so the suite fails
rather than silently passing.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from release_publish_path_consumer_smoke import CONSUMER_SMOKE_JOB

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: One parsed workflow or composite-action document, mutated in place.
Job = dict[str, object]

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

#: The guard clauses, each with the operator that joins it to the previous
#: clause, so removing one leaves an expression the evaluator can still parse.
CANCELLED_CLAUSE = "!cancelled() && "
FINALIZE_CLAUSE = "needs.finalize-github-release.result == 'success' && "
PUBLISH_CLAUSE = "&& needs.metadata.outputs.should_publish == 'true'"
#: A whole build command. ``cargo install netsuke-build`` is what a smoke that
#: compiled instead of downloading would run, and is the substitution the
#: compilation contract exists to refuse.
COMPILATION_STEP = {"name": "Install from source", "run": "cargo install netsuke-build"}
DOWNLOAD_PREFIX = "actions/download-artifact@"
CHECKOUT_PREFIX = "actions/checkout@"


def _smoke_job(release: Job) -> Job:
    """Return the smoke job of a copied release workflow.

    Returns
    -------
    Job
        The mutable job mapping.

    Raises
    ------
    AssertionError
        If the copied workflow no longer declares the job, which means the
        fixture moved and the mutation would test nothing.
    """
    jobs = release.get("jobs")
    assert isinstance(jobs, dict), "the release workflow must declare jobs"
    job = jobs.get(CONSUMER_SMOKE_JOB)
    assert isinstance(job, dict), f"release workflow must declare {CONSUMER_SMOKE_JOB}"
    return job


def _steps(document: Job) -> list[Job]:
    """Return a job's or action's step list for structural mutation.

    Returns
    -------
    list[Job]
        The mutable steps, in declaration order, whether the steps hang off a
        job or off a composite action's ``runs`` mapping.

    Raises
    ------
    AssertionError
        If the document declares no steps.
    """
    runs = document.get("runs")
    holder = runs if isinstance(runs, dict) else document
    declared = holder.get("steps")
    assert isinstance(declared, list), "the mutated document must declare steps"
    for step in declared:
        assert isinstance(step, dict), "every mutated step must be a mapping"
    # The caller mutates this list in place, so it must be the one the document
    # holds. A filtered copy would accept an append and change nothing.
    return typ.cast("list[Job]", declared)


def _without(guard: str, clause: str) -> str:
    """Return ``guard`` with one required clause removed.

    Returns
    -------
    str
        The guard without the clause.

    Raises
    ------
    AssertionError
        If the clause is absent, which means the guard was rewritten and this
        mutation no longer removes anything.
    """
    assert clause in guard, f"the smoke guard must contain {clause!r}, got {guard!r}"
    return guard.replace(clause, "", 1)


def _set_guard(job: Job, clause: str) -> None:
    """Remove ``clause`` from the smoke job's guard."""
    job["if"] = _without(str(job.get("if", "")), clause)


def _permissions(job: Job) -> Job:
    """Return the job's permission mapping for mutation.

    Returns
    -------
    Job
        The mutable permissions.

    Raises
    ------
    AssertionError
        If the job declares no permissions to mutate.
    """
    permissions = job.get("permissions")
    assert isinstance(permissions, dict), f"{CONSUMER_SMOKE_JOB} must set permissions"
    return permissions


def _drop_finalize_need(release: Job, _action: Job) -> None:
    """Remove finalization from the smoke's dependencies and its guard.

    Both halves go together: a dependency deleted from ``needs`` while the
    guard still reads its result is refused by the expression evaluator rather
    than by the dependency contract this mutation targets.
    """
    job = _smoke_job(release)
    needs = job.get("needs")
    assert isinstance(needs, list), f"{CONSUMER_SMOKE_JOB} must declare needs"
    needs.remove("finalize-github-release")
    _set_guard(job, FINALIZE_CLAUSE)


def _ignore_finalize_result(release: Job, _action: Job) -> None:
    """Stop requiring finalization to have succeeded, keeping the dependency."""
    _set_guard(_smoke_job(release), FINALIZE_CLAUSE)


def _remove_publish_guard(release: Job, _action: Job) -> None:
    """Let the smoke run in a mode that publishes nothing."""
    _set_guard(_smoke_job(release), PUBLISH_CLAUSE)


def _remove_cancelled_guard(release: Job, _action: Job) -> None:
    """Drop the explicit cancellation check from the smoke's guard."""
    _set_guard(_smoke_job(release), CANCELLED_CLAUSE)


def _remove_permissions(release: Job, _action: Job) -> None:
    """Remove the job's permission grant entirely."""
    _smoke_job(release).pop("permissions", None)


def _grant_contents_write(release: Job, _action: Job) -> None:
    """Broaden the smoke's repository permission to write."""
    _permissions(_smoke_job(release))["contents"] = "write"


def _add_id_token(release: Job, _action: Job) -> None:
    """Grant the smoke an OIDC token exchange it must never need."""
    _permissions(_smoke_job(release))["id-token"] = "write"


def _add_environment(release: Job, _action: Job) -> None:
    """Bind the smoke to a deployment environment's secrets."""
    _smoke_job(release)["environment"] = "release"


def _add_token_environment(release: Job, _action: Job) -> None:
    """Give the smoke a GitHub token in its environment."""
    _smoke_job(release)["env"] = {"GH_TOKEN": "${{ github.token }}"}


def _persist_checkout_credentials(release: Job, _action: Job) -> None:
    """Leave the workflow token in the job's git configuration.

    This is the mutation the checkout contract exists for: the token is then
    readable by every later step of a job that is supposed to hold none.
    """
    for step in _steps(_smoke_job(release)):
        if not str(step.get("uses", "")).startswith(CHECKOUT_PREFIX):
            continue
        values = step.get("with")
        assert isinstance(values, dict), "the smoke checkout must pass inputs"
        values["persist-credentials"] = True
        return
    message = f"{CONSUMER_SMOKE_JOB} must check the candidate out"
    raise AssertionError(message)


def _select_pattern_artifact(release: Job, _action: Job) -> None:
    """Replace one exact artefact download with a wildcard pattern download."""
    for step in _steps(_smoke_job(release)):
        if str(step.get("uses", "")).startswith(DOWNLOAD_PREFIX):
            step["with"] = {"pattern": "release-*", "path": "downloaded"}
            return
    message = f"{CONSUMER_SMOKE_JOB} must download at least one artefact"
    raise AssertionError(message)


def _compile_in_job(release: Job, _action: Job) -> None:
    """Append the source build a smoke must never run."""
    _steps(_smoke_job(release)).append(dict(COMPILATION_STEP))


def _compile_in_action(release: Job, action: Job) -> None:
    """Append the source build to the action the job calls."""
    _ = release
    _steps(action).append(dict(COMPILATION_STEP))


_MUTATIONS: dict[str, "cabc.Callable[[Job, Job], None]"] = {
    "drop-consumer-smoke-finalize-prerequisite": _drop_finalize_need,
    "ignore-consumer-smoke-finalize-result": _ignore_finalize_result,
    "remove-consumer-smoke-publish-guard": _remove_publish_guard,
    "remove-consumer-smoke-cancelled-guard": _remove_cancelled_guard,
    "remove-consumer-smoke-permissions": _remove_permissions,
    "grant-consumer-smoke-contents-write": _grant_contents_write,
    "add-consumer-smoke-id-token": _add_id_token,
    "add-consumer-smoke-environment": _add_environment,
    "add-consumer-smoke-token-environment": _add_token_environment,
    "persist-consumer-smoke-checkout-credentials": _persist_checkout_credentials,
    "select-consumer-smoke-pattern-artifact": _select_pattern_artifact,
    "compile-in-consumer-smoke-job": _compile_in_job,
    "compile-in-consumer-smoke-action": _compile_in_action,
}


def mutate_consumer_smoke(label: str, release: Job, action: Job) -> None:
    """Apply one named consumer-smoke regression in place.

    Parameters
    ----------
    label
        Name of the registered mutation to apply.
    release
        A deep copy of the release workflow, mutated in place.
    action
        A deep copy of the consumer-smoke manifest, mutated in place.

    Raises
    ------
    AssertionError
        If ``label`` is not registered, or if the fixture it targets has moved
        so far that the mutation would change nothing.
    """
    applier = _MUTATIONS.get(label)
    assert applier is not None, f"unknown consumer-smoke mutation {label!r}"
    applier(release, action)


__all__ = [
    "CONSUMER_SMOKE_EXPECTED_VIOLATIONS",
    "MUTATION_LABELS",
    "mutate_consumer_smoke",
]
