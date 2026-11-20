"""Break the publication gate one clause at a time.

Each mutation states a plausible way to reopen the bypass the gate closes: drop
a dependency, drop one result or output check, invert the guard's cancellation
negation, hard-code the enforcement mode in either direction or in a spelling
the gate refuses, smuggle an ``always()`` onto the publication guard, disjoin
the guard from an always-true clause, add a publisher whose scopes arrive as a
``write-all`` shorthand or through the workflow level, let the rehearsal adopt
the gate, let publication adopt the rehearsal, grant the rehearsal a write
scope the publisher model does not name, or let `setup-uv` resolve the uv
release from the candidate's own project files. The admission job's mutations,
which collapse its two checkouts or move the gate step out of the candidate
checkout, live in :mod:`release_admission_checkout_mutations`, and the
enforcement spellings that keep the mode out of the runner's hands live in
:mod:`release_admission_enforcement_mutations`; both are merged below. The
expected diagnostics name the scenario or the structural rule that
must notice, so the mutation test shows each assertion fires rather than merely
passing.

Run via ``make test-workflow-contracts``.
"""

import re
import typing as typ

from release_admission_checkout_mutations import CHECKOUT_MUTATIONS
from release_admission_enforcement_mutations import ENFORCEMENT_MUTATIONS
from release_admission_gate_contract import (
    ADMISSION_JOB,
    ADMISSION_STEP_ID,
    ENFORCE_VARIABLE,
    SETUP_UV_ACTION,
)
from release_mutation_support import MutationError, MutationIssue, replace_once
from workflow_loading import job_steps, require_list, require_mapping, workflow_job

if typ.TYPE_CHECKING:
    import collections.abc as cabc

PUBLICATION_JOB = "publish-release"
REGISTRY_JOB = "publish-crates-io"
FINALIZE_JOB = "finalize-github-release"
STAGING_JOB = "release"
#: Names the two added-job mutations give the publisher they introduce.
WRITE_ALL_JOB = "publish-packages"
INHERITING_JOB = "publish-notes"

GATE_VIOLATIONS: dict[str, tuple[tuple[str, str], ...]] = {
    "remove-publication-package-need": (
        ("static", "publish-release.needs"),
        ("static", "write-authority.publish-release.needs"),
    ),
    "remove-publication-admission-need": (
        ("static", "publish-release.needs"),
        ("static", "write-authority.publish-release.needs"),
    ),
    "remove-publication-package-result-check": (
        ("publish-package-crate-failure", "publish-job"),
        ("static", "write-authority.publish-release.guard"),
    ),
    "remove-publication-admission-result-check": (
        ("publish-release-admission-canaries-failure", "publish-job"),
        ("static", "write-authority.publish-release.guard"),
    ),
    "remove-publication-enforcement-check": (
        ("tag-publish-report-only-admission", "publish-job"),
        ("static", "write-authority.publish-release.guard"),
    ),
    "remove-publication-outcome-check": (
        ("tag-publish-failed-admission-gate", "publish-job"),
        ("static", "write-authority.publish-release.guard"),
    ),
    "remove-publication-candidate-check": (
        ("tag-publish-mismatched-admission-candidate", "publish-job"),
        ("static", "write-authority.publish-release.guard"),
    ),
    "remove-registry-enforcement-check": (
        ("tag-publish-report-only-admission", "crates-io-job"),
        ("static", "write-authority.publish-crates-io.guard"),
    ),
    "positive-cancellation-publication-guard": (
        ("cancelled-publishing", "publish-job"),
        ("static", "write-authority.publish-release.guard"),
    ),
    "remove-finalize-package-need": (
        ("static", "write-authority.finalize-github-release.needs"),
    ),
    "remove-finalize-verify-result-check": (
        ("verify-release-inventory-failure", "finalize-job"),
    ),
    "finalize-adopts-rehearsal": (
        ("static", "finalize-github-release.needs"),
        ("tag-publish", "finalize-job"),
    ),
    "force-observation-enforcement": (("tag-publish", "admission.enforcement"),),
    "force-enforced-rehearsal": (("opened", "admission.enforcement"),),
    "enforcement-bare-expression": (
        ("tag-publish", "admission.enforcement"),
        ("opened", "admission.enforcement"),
    ),
    "enforcement-uppercase": (
        ("tag-publish", "admission.enforcement"),
        ("opened", "admission.enforcement"),
    ),
    "enforcement-padded": (
        ("tag-publish", "admission.enforcement"),
        ("opened", "admission.enforcement"),
    ),
    "always-publication-guard": (
        ("cancelled-publishing", "publish-job"),
        ("static", "write-authority.publish-release.guard"),
    ),
    "staging-adopts-admission": (("static", "release.needs"),),
    "publication-adopts-staging": (("static", "publish-release.needs"),),
    "disjunctive-publication-guard": (
        ("cancelled-publishing", "publish-job"),
        ("static", "write-authority.publish-release.guard"),
    ),
    "write-all-publisher": (
        ("static", "write-authority.publish-packages.needs"),
        ("static", "write-authority.publish-packages.guard"),
    ),
    "inherited-write-publisher": (
        ("static", "write-authority.publish-notes.needs"),
        ("static", "write-authority.publish-notes.guard"),
    ),
    "grant-rehearsal-unmodelled-write": (("static", "release.permissions"),),
    "caller-selects-gate-implementation": (
        ("static", f"{ADMISSION_JOB}.checkout.verifier"),
    ),
    "gate-from-caller-repository": (("static", f"{ADMISSION_JOB}.checkout.verifier"),),
    "gate-from-run-commit-only": (("static", f"{ADMISSION_JOB}.checkout.verifier"),),
    "single-pinned-checkout": (("static", f"{ADMISSION_JOB}.checkout"),),
    "gate-checkout-replaces-candidate": (
        ("static", f"{ADMISSION_JOB}.checkout.verifier"),
    ),
    "candidate-checkout-names-verifier": (
        ("static", f"{ADMISSION_JOB}.checkout.candidate"),
    ),
    "candidate-checkout-conditional": (
        ("static", f"{ADMISSION_JOB}.checkout.candidate"),
    ),
    "candidate-checkout-tolerates-failure": (
        ("static", f"{ADMISSION_JOB}.checkout.candidate"),
    ),
    "verifier-checkout-conditional": (
        ("static", f"{ADMISSION_JOB}.checkout.verifier"),
    ),
    "verifier-checkout-tolerates-failure": (
        ("static", f"{ADMISSION_JOB}.checkout.verifier"),
    ),
    "verifier-persists-credentials": (
        ("static", f"{ADMISSION_JOB}.checkout.verifier"),
    ),
    "verifier-path-needs-quoting": (("static", f"{ADMISSION_JOB}.checkout.verifier"),),
    "checkouts-swapped": (("static", f"{ADMISSION_JOB}.checkout.candidate"),),
    "gate-runs-candidate-script": (("static", f"{ADMISSION_JOB}.checkout.gate"),),
    "gate-step-leaves-candidate": (("static", f"{ADMISSION_JOB}.checkout.gate"),),
    "job-default-moves-gate": (("static", f"{ADMISSION_JOB}.checkout.gate"),),
    "workflow-default-moves-gate": (("static", f"{ADMISSION_JOB}.checkout.gate"),),
    "gate-run-splits-the-command": (("static", f"{ADMISSION_JOB}.checkout.gate"),),
    "gate-run-without-uv-isolation": (("static", f"{ADMISSION_JOB}.checkout.gate"),),
    "admission-uv-unpinned": (("static", f"{ADMISSION_JOB}.setup-uv"),),
}


def _needs_of(release: dict[str, object], job_name: str) -> list[object]:
    """Return one job's declared dependency list, refusing a malformed shape."""
    return require_list(
        workflow_job(release, job_name).get("needs"), f"{job_name}.needs"
    )


def _drop_need(release: dict[str, object], job_name: str, need: str) -> None:
    """Remove one dependency, refusing a rule whose target has moved."""
    needs = _needs_of(release, job_name)
    if need not in needs:
        raise MutationError(MutationIssue.PATTERN_MISMATCH, (job_name, need))
    needs.remove(need)


def _drop_guard_clause(release: dict[str, object], job_name: str, clause: str) -> None:
    """Remove one guard clause, leaving every other clause as written."""
    job = workflow_job(release, job_name)
    pattern = rf"{re.escape(' '.join(clause.split()))}\s*&&\s*"
    job["if"] = replace_once(str(job.get("if", "")), pattern, "")


def _admission_environment(release: dict[str, object]) -> dict[str, object]:
    """Return the gate step's environment, refusing a missing step."""
    step = next(
        (
            item
            for item in job_steps(release, ADMISSION_JOB)
            if item.get("id") == ADMISSION_STEP_ID
        ),
        None,
    )
    if step is None:
        raise MutationError(MutationIssue.PATTERN_MISMATCH, ADMISSION_STEP_ID)
    return require_mapping(step.get("env"), f"{ADMISSION_JOB} step environment")


def _set_enforcement(release: dict[str, object], literal: str) -> None:
    """Configure enforcement beside the release mode instead of deriving it."""
    _admission_environment(release)[ENFORCE_VARIABLE] = literal


def _admission_uv_unpinned(release: dict[str, object]) -> None:
    """Install whatever uv release the candidate's project files resolve.

    `setup-uv` defaults its working directory to the workspace root, which the
    candidate checkout owns, and an unset ``version`` resolves from a
    ``uv.toml`` or ``pyproject.toml`` it finds there. A candidate can then pick
    the uv release that runs a gate holding this job's token. The action commit
    stays pinned: this is the input that decides the binary, not the installer.

    Raises
    ------
    MutationError
        If the admission job's setup step already names no version, so the
        input this mutation removes is absent and the case cannot be stated.
    """
    step = next(
        (
            item
            for item in job_steps(release, ADMISSION_JOB)
            if str(item.get("uses", "")).startswith(f"{SETUP_UV_ACTION}@")
        ),
        None,
    )
    if step is None:
        raise MutationError(MutationIssue.PATTERN_MISMATCH, SETUP_UV_ACTION)
    declared = require_mapping(step.get("with"), f"{ADMISSION_JOB} setup-uv inputs")
    if declared.pop("version", None) is None:
        raise MutationError(MutationIssue.PATTERN_MISMATCH, "version")


def _always_publication_guard(release: dict[str, object]) -> None:
    """Let the publication job start even when the run was cancelled."""
    job = workflow_job(release, PUBLICATION_JOB)
    job["if"] = replace_once(
        str(job.get("if", "")), r"!cancelled\(\)\s*&&\s*", "always() && "
    )


def _cancelled_publication_guard(release: dict[str, object]) -> None:
    """Start publication when the run was cancelled, keeping every gate clause.

    ``cancelled()`` is true after a run is cancelled, and the dependencies the
    rest of the guard names may all have succeeded by then, so the conjunction
    is satisfied exactly when publication must not happen.
    """
    job = workflow_job(release, PUBLICATION_JOB)
    job["if"] = replace_once(str(job.get("if", "")), r"!cancelled\(\)", "cancelled()")


def _staging_adopts_admission(release: dict[str, object]) -> None:
    """Make the rehearsal wait on the gate it exists to survive."""
    _needs_of(release, STAGING_JOB).append(ADMISSION_JOB)


def _publication_adopts_staging(release: dict[str, object]) -> None:
    """Make publication wait on the rehearsal job, which a dry run alone runs."""
    _needs_of(release, PUBLICATION_JOB).append(STAGING_JOB)


def _disjunctive_publication_guard(release: dict[str, object]) -> None:
    """Authorize publication alone, keeping every gate clause as a conjunct.

    ``&&`` binds tighter than ``||``, so appending an always-true disjunct
    leaves the guard always true while every required clause still appears at
    the top level. A clause-matching reader that never looks for a disjunction
    reads this as fully gated.
    """
    job = workflow_job(release, PUBLICATION_JOB)
    job["if"] = f"{job.get('if', '')} || true"


def _add_job(release: dict[str, object], name: str, job: dict[str, object]) -> None:
    """Add one job, refusing a rule whose name is already taken."""
    jobs = require_mapping(release.get("jobs"), "release.jobs")
    if name in jobs:
        raise MutationError(MutationIssue.PATTERN_MISMATCH, name)
    jobs[name] = job


def _write_all_publisher(release: dict[str, object]) -> None:
    """Add a publisher whose scope shorthand a mapping-only reader would miss."""
    _add_job(release, WRITE_ALL_JOB, {"permissions": "write-all"})


def _inherited_write_publisher(release: dict[str, object]) -> None:
    """Grant write at the top level and add a job that inherits it."""
    release["permissions"] = {"contents": "write"}
    _add_job(release, INHERITING_JOB, {"runs-on": "ubuntu-latest"})


def _grant_unmodelled_write(release: dict[str, object]) -> None:
    """Grant a job that must stay reachable a write scope outside the model.

    ``actions`` cannot publish a release, so the publisher model does not name
    it; the read-only rule is about what the job may write at all, so this must
    be reported rather than read as no write.
    """
    workflow_job(release, STAGING_JOB)["permissions"] = {"actions": "write"}


#: Each labelled clause removal, with the job whose guard loses it.
CLAUSE_REMOVALS: dict[str, tuple[str, str]] = {
    "remove-publication-package-result-check": (
        PUBLICATION_JOB,
        "needs.package-crate.result == 'success'",
    ),
    "remove-publication-admission-result-check": (
        PUBLICATION_JOB,
        "needs.release-admission-canaries.result == 'success'",
    ),
    "remove-publication-enforcement-check": (
        PUBLICATION_JOB,
        "needs.release-admission-canaries.outputs.enforcement == 'true'",
    ),
    "remove-publication-outcome-check": (
        PUBLICATION_JOB,
        "needs.release-admission-canaries.outputs.gate_outcome == 'success'",
    ),
    "remove-publication-candidate-check": (
        PUBLICATION_JOB,
        "needs.release-admission-canaries.outputs.candidate_commit == github.sha",
    ),
    "remove-registry-enforcement-check": (
        REGISTRY_JOB,
        "needs.release-admission-canaries.outputs.enforcement == 'true'",
    ),
    "remove-finalize-verify-result-check": (
        FINALIZE_JOB,
        "needs.verify-release-inventory.result == 'success'",
    ),
}


def _finalize_adopts_rehearsal(release: dict[str, object]) -> None:
    """Make finalization wait on the rehearsal job, which a dry run alone runs.

    The guard keeps every clause it needs and gains one more, so a clause-set
    reader sees nothing wrong. What it cannot see is that the rehearsal is
    skipped on every publishing run, which leaves finalization permanently
    unreachable -- a release that publishes and is never un-drafted.
    """
    _needs_of(release, FINALIZE_JOB).append(STAGING_JOB)
    job = workflow_job(release, FINALIZE_JOB)
    job["if"] = f"{job.get('if', '')} && needs.release.result == 'success'"


def _drop_clause_for(label: str, release: dict[str, object]) -> None:
    """Apply one of the guard-clause removals named in :data:`CLAUSE_REMOVALS`."""
    job_name, clause = CLAUSE_REMOVALS[label]
    _drop_guard_clause(release, job_name, clause)


GATE_MUTATIONS: dict[str, cabc.Callable[[dict[str, object]], None]] = {
    "remove-publication-package-need": lambda release: _drop_need(
        release, PUBLICATION_JOB, "package-crate"
    ),
    "remove-publication-admission-need": lambda release: _drop_need(
        release, PUBLICATION_JOB, ADMISSION_JOB
    ),
    "remove-finalize-package-need": lambda release: _drop_need(
        release, FINALIZE_JOB, "package-crate"
    ),
    "finalize-adopts-rehearsal": _finalize_adopts_rehearsal,
    "force-observation-enforcement": lambda release: _set_enforcement(release, "false"),
    "force-enforced-rehearsal": lambda release: _set_enforcement(release, "true"),
    "admission-uv-unpinned": _admission_uv_unpinned,
    "always-publication-guard": _always_publication_guard,
    "positive-cancellation-publication-guard": _cancelled_publication_guard,
    "staging-adopts-admission": _staging_adopts_admission,
    "publication-adopts-staging": _publication_adopts_staging,
    "disjunctive-publication-guard": _disjunctive_publication_guard,
    "write-all-publisher": _write_all_publisher,
    "inherited-write-publisher": _inherited_write_publisher,
    "grant-rehearsal-unmodelled-write": _grant_unmodelled_write,
    **CHECKOUT_MUTATIONS,
    **ENFORCEMENT_MUTATIONS,
    **{
        label: (lambda release, name=label: _drop_clause_for(name, release))
        for label in CLAUSE_REMOVALS
    },
}

__all__ = ("GATE_MUTATIONS", "GATE_VIOLATIONS")
