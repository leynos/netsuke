"""Hold every write-authority release job to the publication gate.

The release workflow's write scopes name the places that can change a release,
the registry, or the repository's packages. Listing them one by one would let a
new publisher be added beside the gate rather than behind it, so this module
derives the set from the workflow itself and holds every member to the same
rule: depend on packaging and on the gate, require both results to be
successful, require enforcement, a successful outcome and the run's own
candidate, and leave no ``always()`` or disjunction to override any of it.

A job's scopes come from its own declaration, or from the workflow's when the
job declares none, and either may be the ``write-all`` shorthand. A job whose
scopes cannot be derived at all is reported rather than assumed read-only,
because the repository default can grant write.

It also holds the two shapes the gate cannot protect on its own. The jobs that
must stay reachable whatever admission decides -- the gate, packaging and the
rehearsal -- may hold no write scope of any kind, not merely none of the three
the publisher model names, and the caller workflow must pass the rehearsal flag
without a publish request or inherited secrets.

Run via ``make test-workflow-contracts``.
"""

import re

from actions_expressions import (
    contains_unquoted_or,
    top_level_conjuncts,
    unwrap_expression,
)
from release_admission_gate_contract import ADMISSION_JOB
from release_publish_needs import job_names
from workflow_loading import require_mapping, workflow_job

#: The scopes that allow a job to change something. `contents` covers releases,
#: `packages` covers the package registry, and `id-token` is the OIDC exchange
#: the crates.io publisher trades for a registry token.
WRITE_SCOPES = ("contents", "id-token", "packages")
#: The two shorthands a `permissions` declaration may use instead of a mapping.
_SHORTHANDS: dict[str, str] = {"read-all": "read", "write-all": "write"}
PACKAGE_JOB = "package-crate"
#: The jobs that must remain read-only. Each either runs before admission has
#: decided, or keeps a failed package or a refused admission from hiding the
#: dry run.
READ_ONLY_JOBS = (ADMISSION_JOB, PACKAGE_JOB, "release")
REQUIRED_DEPENDENCIES = (PACKAGE_JOB, ADMISSION_JOB)
#: Every top-level clause a write job's guard must carry. A clause inside a
#: negation is not a top-level conjunct, so `a && !(b)` is reported rather than
#: read as present. A disjunction anywhere in the guard is refused outright:
#: `&&` binds tighter than `||`, so `a && b && false || true` keeps every
#: clause as a conjunct and is still always true.
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
        at write, however the declaration spelled it.
    """
    jobs = require_mapping(release.get("jobs"), "release.jobs")
    found: dict[str, dict[str, object]] = {}
    for name, job in jobs.items():
        if not isinstance(job, dict):
            continue
        permissions = effective_permissions(release, job)
        if permissions is not None and _holds_write(permissions):
            found[str(name)] = job
    return found


def effective_permissions(
    release: dict[str, object], job: dict[str, object]
) -> dict[str, object] | None:
    """Return the scopes a job really holds, or ``None`` when undeclared.

    GitHub resolves a job's permissions in two steps. A job that declares its
    own wins outright; one that does not inherits the workflow-level
    declaration. Either may be the ``write-all`` shorthand rather than a
    mapping, and a shorthand grants every scope at its level. Returning
    ``None`` for a job whose scopes cannot be derived keeps an omitted
    workflow-level declaration from reading as "no write": the repository's
    own default could grant it.

    Parameters
    ----------
    release
        Parsed release workflow.
    job
        One job from that workflow.

    Returns
    -------
    dict[str, object] | None
        The granted scopes, or ``None`` when neither the job nor the workflow
        declares any.
    """
    declared = (
        job["permissions"] if "permissions" in job else release.get("permissions")
    )
    if isinstance(declared, dict):
        return declared
    level = _SHORTHANDS.get(declared) if isinstance(declared, str) else None
    if level is None:
        return None
    return dict.fromkeys(WRITE_SCOPES, level)


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

    Every scope the job declares is inspected, not only the three the publisher
    model names. Those three are what a job needs to change a release; the rule
    here is stronger, because a job that must stay reachable may write nothing
    at all, and a scope outside the model -- ``actions``, say -- is still a
    permission the job did not need.

    Parameters
    ----------
    release
        Parsed release workflow.
    violations
        Collector for every job's findings.
    """
    for name in READ_ONLY_JOBS:
        permissions = effective_permissions(release, workflow_job(release, name))
        if permissions is None:
            violations.append(
                f"{name}.permissions: cannot derive the job's scopes; declare them "
                "on the job or the workflow"
            )
            continue
        granted = sorted(
            scope for scope, level in permissions.items() if level == "write"
        )
        if granted:
            violations.append(
                f"{name}.permissions: must hold no write scope, got {granted!r}"
            )


def _check_write_authority(release: dict[str, object], violations: list[str]) -> None:
    """Hold every write job to packaging, the gate and the absence of bypasses."""
    _check_permissions_declared(release, violations)
    jobs = write_authority_jobs(release)
    if not jobs:
        violations.append("write-authority: no release job holds a write scope")
        return
    for name, job in sorted(jobs.items()):
        _check_write_needs(name, job, violations)
        check_write_guard(name, job, violations)


def _check_permissions_declared(
    release: dict[str, object], violations: list[str]
) -> None:
    """Require every job's scopes to be derivable from its own or the top level."""
    jobs = require_mapping(release.get("jobs"), "release.jobs")
    violations.extend(
        f"write-authority.{name}.permissions: cannot derive the job's scopes; "
        "declare them on the job or the workflow"
        for name, job in jobs.items()
        if isinstance(job, dict) and effective_permissions(release, job) is None
    )


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


def check_write_guard(name: str, job: dict[str, object], violations: list[str]) -> None:
    """Require every gate clause as a top-level conjunct, and no status bypass.

    The optional ``${{ }}`` wrapper is removed first, because GitHub accepts
    the wrapped and bare spellings of one guard and both must be judged the
    same way. Leaving it on would attach ``}}`` to the final clause and report
    a check the guard does carry as missing.

    Parameters
    ----------
    name
        The job's name, used to label each finding.
    job
        One release job.
    violations
        Collector for every finding.
    """
    guard = " ".join(unwrap_expression(str(job.get("if", ""))).split())
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
    if contains_unquoted_or(guard):
        violations.append(
            f"write-authority.{name}.guard: must be a conjunction, "
            "with no disjunction to authorize the job alone"
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
    "check_write_guard",
    "effective_permissions",
    "write_authority_jobs",
)
