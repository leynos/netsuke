"""Hold every write-authority release job to the publication gate.

The release workflow's write scopes name the places that can change a release,
the registry, or the repository's packages. Listing them one by one would let a
new publisher be added beside the gate rather than behind it, so this module
derives the set from the workflow itself and holds every member to the same
rule: depend on packaging and on the gate, require both results to be
successful, require enforcement, a successful outcome and the run's own
candidate, and leave no ``always()`` to override any of it.

It also holds the two shapes the gate cannot protect on its own. The jobs that
must stay reachable whatever admission decides -- the gate, packaging and the
rehearsal -- may hold no write scope, and the caller workflow must pass the
rehearsal flag without a publish request or inherited secrets.

Run via ``make test-workflow-contracts``.
"""

import re

from actions_expressions import top_level_conjuncts
from release_admission_gate_contract import ADMISSION_JOB
from release_publish_needs import job_names
from workflow_loading import require_mapping, workflow_job

#: The scopes that allow a job to change something. `contents` covers releases,
#: `packages` covers the package registry, and `id-token` is the OIDC exchange
#: the crates.io publisher trades for a registry token.
WRITE_SCOPES = ("contents", "id-token", "packages")
PACKAGE_JOB = "package-crate"
#: The jobs that must remain read-only. Each either runs before admission has
#: decided, or keeps a failed package or a refused admission from hiding the
#: dry run.
READ_ONLY_JOBS = (ADMISSION_JOB, PACKAGE_JOB, "release")
REQUIRED_DEPENDENCIES = (PACKAGE_JOB, ADMISSION_JOB)
#: Every top-level clause a write job's guard must carry. A clause inside a
#: disjunction or a negation is not a top-level conjunct, so `|| true` and
#: `a && !(b)` are both reported rather than read as present.
REQUIRED_GUARD_CLAUSES = (
    *(f"needs.{name}.result == 'success'" for name in REQUIRED_DEPENDENCIES),
    f"needs.{ADMISSION_JOB}.outputs.enforcement == 'true'",
    f"needs.{ADMISSION_JOB}.outputs.gate_outcome == 'success'",
    f"needs.{ADMISSION_JOB}.outputs.candidate_commit == github.sha",
)
#: Any call, negated or not, changes what starts the job: `always()` ignores
#: cancellation, which is what the explicit `!cancelled()` clause exists to
#: express beside the result checks.
_ALWAYS = re.compile(r"\balways\s*\(")
CALLER_JOB = "release"


def write_authority_jobs(
    release: dict[str, object],
) -> dict[str, dict[str, object]]:
    """Return every release job that holds a write scope.

    Returns
    -------
    dict[str, dict[str, object]]
        One entry per job granting ``contents``, ``packages`` or ``id-token``
        at write.
    """
    jobs = require_mapping(release.get("jobs"), "release.jobs")
    found: dict[str, dict[str, object]] = {}
    for name, job in jobs.items():
        if not isinstance(job, dict):
            continue
        permissions = job.get("permissions")
        if isinstance(permissions, dict) and _holds_write(permissions):
            found[str(name)] = job
    return found


def _holds_write(permissions: dict[str, object]) -> bool:
    """Return whether any modelled scope is granted at write."""
    return any(permissions.get(scope) == "write" for scope in WRITE_SCOPES)


def check_publication_gate(
    release: dict[str, object],
    caller: dict[str, object],
    violations: list[str],
) -> None:
    """Check every structural rule the publication gate rests on.

    Parameters
    ----------
    release
        Parsed release workflow.
    caller
        Parsed dry-run caller workflow.
    violations
        Collector for every finding.
    """
    check_read_only_jobs(release, violations)
    _check_write_authority(release, violations)
    check_rehearsal_caller(caller, violations)


def check_read_only_jobs(release: dict[str, object], violations: list[str]) -> None:
    """Require the gate, packaging and the rehearsal to hold no write scope.

    Parameters
    ----------
    release
        Parsed release workflow.
    violations
        Collector for every job's findings.
    """
    for name in READ_ONLY_JOBS:
        permissions = workflow_job(release, name).get("permissions", {})
        if not isinstance(permissions, dict):
            violations.append(f"{name}.permissions: must be a mapping")
            continue
        granted = sorted(
            scope for scope in WRITE_SCOPES if permissions.get(scope) == "write"
        )
        if granted:
            violations.append(
                f"{name}.permissions: must hold no write scope, got {granted!r}"
            )


def _check_write_authority(release: dict[str, object], violations: list[str]) -> None:
    """Hold every write job to packaging, the gate and the absence of bypasses."""
    jobs = write_authority_jobs(release)
    if not jobs:
        violations.append("write-authority: no release job holds a write scope")
        return
    for name, job in sorted(jobs.items()):
        _check_write_needs(name, job, violations)
        _check_write_guard(name, job, violations)


def _check_write_needs(
    name: str, job: dict[str, object], violations: list[str]
) -> None:
    """Require a write job to declare packaging and the admission gate."""
    needs = job_names(job.get("needs", []))
    if needs is None:
        violations.append(
            f"write-authority.{name}.needs: dependencies must be job names"
        )
        return
    missing = sorted(set(REQUIRED_DEPENDENCIES) - set(needs))
    if missing:
        violations.append(f"write-authority.{name}.needs: missing {missing!r}")


def _check_write_guard(
    name: str, job: dict[str, object], violations: list[str]
) -> None:
    """Require every gate clause as a top-level conjunct, and no status bypass."""
    guard = " ".join(str(job.get("if", "")).split())
    clauses = set(top_level_conjuncts(guard))
    violations.extend(
        f"write-authority.{name}.guard: missing top-level check {clause!r}"
        for clause in REQUIRED_GUARD_CLAUSES
        if clause not in clauses
    )
    if _ALWAYS.search(guard):
        violations.append(
            f"write-authority.{name}.guard: must not call always() over the gate"
        )


def check_rehearsal_caller(caller: dict[str, object], violations: list[str]) -> None:
    """Require the caller to request a rehearsal and nothing more.

    Parameters
    ----------
    caller
        Parsed dry-run caller workflow.
    violations
        Collector for every finding.
    """
    job = workflow_job(caller, CALLER_JOB)
    inputs = require_mapping(job.get("with"), "caller release.with")
    if inputs.get("dry-run") is not True:
        violations.append("caller.dry-run: the rehearsal must pass dry-run true")
    if "publish" in inputs:
        violations.append("caller.publish: the rehearsal must not request publish")
    if "secrets" in job:
        violations.append(
            "caller.secrets: the rehearsal must inherit no release secrets"
        )


__all__ = (
    "READ_ONLY_JOBS",
    "REQUIRED_DEPENDENCIES",
    "REQUIRED_GUARD_CLAUSES",
    "WRITE_SCOPES",
    "check_publication_gate",
    "check_read_only_jobs",
    "check_rehearsal_caller",
    "write_authority_jobs",
)
