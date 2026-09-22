"""What a pull-request-reachable workflow may not touch.

Pull-request CI generates `lcov.info` and compares it with the ratcheted
baseline derived from `main`. It does not publish that report as an artefact,
invoke the CodeScene coverage action, or carry the CodeScene credential; those
belong to `coverage-main.yml`, which is the only writer of persistent coverage
state.

The coverage action archives the report it generated under a step of its own,
so declining that archive is part of the same boundary: a caller that reaches
the action without the opt-out has published the report whether or not the
workflow declares an artefacts step. That rule is checked here rather than in
the workflow, because the action's own step is not the caller's to see.

A pull request reaches CodeScene by more routes than the action. A step can
curl the service directly, naming neither the action nor the credential, so the
service's host is refused in its own right. And ``secrets: inherit`` hands a
called workflow every secret without naming one: to a workflow in this
repository that is visible, because the closure in ``workflow_call_closure``
reads the callee too, but to another repository's workflow it is not, so that
call is refused.

These predicates read parsed workflow values and raw text rather than files, so
``ci_coverage_wiring_test`` can hold every workflow a pull request reaches, and
any ``workflow_run`` consumer, to the boundary, and can drive shapes the
repository does not have.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

from codescene_check_depth_invariants import CODESCENE_COVERAGE_ACTION
from runner_placement_invariants import contains_unquoted_or
from timeout_budgets import COVERAGE_ACTION
from workflow_call_closure import (
    called_workflows,
    local_workflow_name,
    reachable_workflows,
)
from workflow_loading import require_mapping

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The action that generates coverage. A pull request calls it in ratchet mode
#: and stops there; `main` calls it to produce the report it publishes. Defined
#: beside the watchdog that caps its `cargo` invocation.
GENERATE_COVERAGE_ACTION: typ.Final[str] = COVERAGE_ACTION

#: The action that submits a report to CodeScene, in either of its modes.
#: `main` owns this call. Defined beside the depth its `check` mode needs.
UPLOAD_COVERAGE_ACTION: typ.Final[str] = CODESCENE_COVERAGE_ACTION

#: The generic artefact action. A pull request must not carry the report to it
#: under any step name.
PUBLISH_ARTEFACT_ACTION: typ.Final[str] = "actions/upload-artifact"

#: The input that suppresses the coverage action's own archive step, and the
#: value that suppresses it. A pull-request-reachable caller must set both, or
#: the action publishes the report this boundary exists to keep local.
PUBLICATION_OPT_OUT_INPUT: typ.Final[str] = "publish-artefact"
PUBLICATION_OPT_OUT_VALUE: typ.Final[str] = "false"

#: The credential the CodeScene upload reads. It must not appear in a workflow
#: a pull request can reach, in a parsed value or anywhere in the raw text.
CREDENTIAL_ENVIRONMENT_KEY: typ.Final[str] = "CS_ACCESS_TOKEN"

#: The service's host. Matched case-insensitively, because DNS names are, and
#: kept apart from the credential check: a step can reach the project API by
#: curling it, naming neither the action, the client, nor the credential.
CODESCENE_HOST: typ.Final[str] = "codescene.io"

#: The two conjuncts the CodeScene upload's ``if`` must carry. The token
#: clause lets a fork's push skip the step; the ref clause keeps a warm-run
#: dispatch from a feature branch from uploading that branch's report.
UPLOAD_GUARD_CONJUNCTS: typ.Final[frozenset[str]] = frozenset({
    f"env.{CREDENTIAL_ENVIRONMENT_KEY} != ''",
    "github.ref == 'refs/heads/main'",
})

#: The ``secrets:`` value that forwards every secret the caller holds.
INHERIT_ALL_SECRETS: typ.Final[str] = "inherit"

#: The report the coverage action writes, and the one CodeScene is sent.
COVERAGE_REPORT_PATH: typ.Final[str] = "lcov.info"

PULL_REQUEST_TRIGGER: typ.Final[str] = "pull_request"

#: The variant that runs in the base repository's context and therefore *can*
#: read its secrets, unlike `pull_request`. A coverage step here would be worse
#: than one in an ordinary pull-request job, not equivalent to it.
PULL_REQUEST_TARGET_TRIGGER: typ.Final[str] = "pull_request_target"

#: The trigger that resumes a run with the base repository's privileges.
SUBMISSION_TRIGGER: typ.Final[str] = "workflow_run"


def action_of(step: dict[str, object]) -> str:
    """Return a step's action reference without its version.

    Splitting on the version separator rather than matching a prefix keeps
    ``upload-codescene-coverage-legacy`` from reading as the real action.

    Parameters
    ----------
    step : dict[str, object]
        One parsed workflow step.

    Returns
    -------
    str
        The action reference without its version, or the empty string when
        the step runs a command instead of an action.
    """
    uses = step.get("uses")
    return uses.split("@", 1)[0] if isinstance(uses, str) else ""


def declares_trigger(document: dict[str, object], trigger: str) -> bool:
    """Return whether a parsed workflow declares the given trigger.

    Parameters
    ----------
    document : dict[str, object]
        One parsed workflow document.
    trigger : str
        The trigger name, such as ``pull_request``.

    Returns
    -------
    bool
        True when the workflow declares the trigger in any of the scalar,
        sequence, or mapping forms ``on:`` accepts.
    """
    match document.get("on"):
        case str() as declared:
            return declared == trigger
        case list() as sequence:
            return trigger in sequence
        case dict() as mapping:
            return trigger in mapping
        case _:
            return False


def pull_request_lane(
    documents: cabc.Mapping[str, dict[str, object]],
) -> frozenset[str]:
    """Return every workflow a pull request runs, by file name.

    Parameters
    ----------
    documents : Mapping[str, dict[str, object]]
        Every workflow document, keyed by file name.

    Returns
    -------
    frozenset[str]
        The workflows declaring either pull-request trigger, and every
        workflow they call, transitively. A local call naming a workflow
        ``documents`` does not hold propagates
        ``UnresolvedWorkflowCallError`` from the traversal.
    """
    entries = [
        name
        for name, document in documents.items()
        if declares_trigger(document, PULL_REQUEST_TRIGGER)
        or declares_trigger(document, PULL_REQUEST_TARGET_TRIGGER)
    ]
    return reachable_workflows(documents, entries)


def steps_in_all_jobs(document: dict[str, object]) -> list[dict[str, object]]:
    """Return every step of every job in one parsed workflow.

    Parameters
    ----------
    document : dict[str, object]
        One parsed workflow document.

    Returns
    -------
    list[dict[str, object]]
        Every step mapping, in declaration order. A job without a step list
        contributes nothing rather than failing, because this is a scan for
        prohibited references and not an assertion about job shape.
    """
    jobs = require_mapping(document.get("jobs"), "jobs")
    steps: list[dict[str, object]] = []
    for name, declaration in jobs.items():
        job = require_mapping(declaration, f"job {name}")
        raw_steps = job.get("steps", [])
        if isinstance(raw_steps, list):
            steps.extend(step for step in raw_steps if isinstance(step, dict))
    return steps


def _iter_strings(value: object) -> cabc.Iterator[str]:
    """Yield every string nested anywhere in a parsed YAML value."""
    match value:
        case str() as text:
            yield text
        case dict() as mapping:
            for key, item in mapping.items():
                yield from _iter_strings(key)
                yield from _iter_strings(item)
        case list() as sequence:
            for item in sequence:
                yield from _iter_strings(item)
        case _:
            return


def publishes_the_coverage_report(step: dict[str, object]) -> bool:
    """Return whether a step publishes the coverage report as an artefact.

    Parameters
    ----------
    step : dict[str, object]
        One parsed workflow step.

    Returns
    -------
    bool
        True when the step uploads the report. A step of the artefact action
        that names no path uploads the workspace, which holds the generated
        report, so it fails closed rather than reading as an exemption.
    """
    if action_of(step) != PUBLISH_ARTEFACT_ACTION:
        return False
    with_ = step.get("with")
    if not isinstance(with_, dict) or "path" not in with_:
        return True
    return COVERAGE_REPORT_PATH in str(with_["path"])


def declines_the_generated_report_archive(step: dict[str, object]) -> bool:
    """Return whether a step tells the coverage action not to archive.

    Parameters
    ----------
    step : dict[str, object]
        One parsed workflow step.

    Returns
    -------
    bool
        True when the step invokes the coverage action and passes the
        publication opt-out. The value is compared as the string the action
        itself compares against, so ``false``, not a falsy stand-in, is what
        suppresses the upload.
    """
    if action_of(step) != GENERATE_COVERAGE_ACTION:
        return False
    with_ = step.get("with")
    if not isinstance(with_, dict):
        return False
    return with_.get(PUBLICATION_OPT_OUT_INPUT) == PUBLICATION_OPT_OUT_VALUE


def is_trunk_only_upload(condition: object) -> bool:
    """Return whether an upload condition requires the token and the trunk ref.

    Any unquoted ``||`` is refused first, at any depth. ``&&`` binds tighter
    than ``||`` in an Actions expression, so in
    ``github.event_name == 'workflow_dispatch' || github.ref == 'refs/heads/main'
    && ...`` the first disjunct authorizes the upload alone however complete
    the rest is. The condition is then split on ``&&``, and every guard clause
    must be one of the conjuncts, compared whole. A substring test would accept
    a clause that is present but negated or nested. Further conjuncts are
    allowed, because without a disjunction they can only narrow the step.
    Parentheses are not interpreted: a clause wrapped in them does not equal
    its bare form, so the reading fails closed.

    Parameters
    ----------
    condition : object
        The upload step's ``if`` value, as parsed.

    Returns
    -------
    bool
        True when the condition has no disjunction and carries every guard
        clause as a conjunct, in any order and spacing.

    Examples
    --------
    >>> token, main = "env.CS_ACCESS_TOKEN != ''", "github.ref == 'refs/heads/main'"
    >>> is_trunk_only_upload(f"{token} && {main}")
    True
    >>> is_trunk_only_upload("env.CS_ACCESS_TOKEN != ''")
    False
    """
    if not isinstance(condition, str):
        return False
    normalized = " ".join(condition.split())
    if contains_unquoted_or(normalized):
        return False
    conjuncts = {part.strip() for part in normalized.split("&&")}
    return conjuncts >= UPLOAD_GUARD_CONJUNCTS


def forwards_every_secret_elsewhere(document: dict[str, object]) -> list[str]:
    """Return the jobs that hand every secret to another repository's workflow.

    Parameters
    ----------
    document : dict[str, object]
        One parsed workflow document.

    Returns
    -------
    list[str]
        The names of jobs whose call is not local and passes
        ``secrets: inherit``. A local call is not listed: the closure reads its
        callee, so whatever that workflow does with a secret is checked there.
    """
    jobs = require_mapping(document.get("jobs"), "jobs")
    return [
        name
        for name, reference in called_workflows(document)
        if local_workflow_name(reference) is None
        and require_mapping(jobs[name], f"job {name}").get("secrets")
        == INHERIT_ALL_SECRETS
    ]


def coverage_surface_offenders(
    name: str, document: dict[str, object], raw_text: str
) -> list[str]:
    """Return every prohibited coverage-surface reference in one workflow.

    Parameters
    ----------
    name : str
        The workflow file's name, used in failure messages.
    document : dict[str, object]
        The workflow's parsed document.
    raw_text : str
        The workflow's raw text. The credential and the service's host are
        matched here, the credential in the parsed values as well, so a
        reference inside a comment or an unparsed shape is still reported.

    Returns
    -------
    list[str]
        One description per violation, empty when the workflow is clean.
    """
    steps = steps_in_all_jobs(document)
    offenders = [
        f"{name}: step {index} publishes the coverage report as an artefact"
        for index, step in enumerate(steps)
        if publishes_the_coverage_report(step)
    ]
    offenders.extend(
        f"{name}: step {index} invokes the coverage action without declining "
        f"its own archive ({PUBLICATION_OPT_OUT_INPUT}: "
        f"{PUBLICATION_OPT_OUT_VALUE})"
        for index, step in enumerate(steps)
        if action_of(step) == GENERATE_COVERAGE_ACTION
        and not declines_the_generated_report_archive(step)
    )
    offenders.extend(
        f"{name}: step {index} invokes the CodeScene coverage action"
        for index, step in enumerate(steps)
        if action_of(step) == UPLOAD_COVERAGE_ACTION
    )
    if CREDENTIAL_ENVIRONMENT_KEY in raw_text:
        offenders.append(f"{name}: raw text references {CREDENTIAL_ENVIRONMENT_KEY}")
    offenders.extend(
        f"{name}: parsed value references {CREDENTIAL_ENVIRONMENT_KEY}"
        for value in _iter_strings(document)
        if CREDENTIAL_ENVIRONMENT_KEY in value
    )
    if CODESCENE_HOST in raw_text.casefold():
        offenders.append(f"{name}: raw text contacts {CODESCENE_HOST}")
    offenders.extend(
        f"{name}: job {job} forwards every secret to another repository's "
        f"workflow ({INHERIT_ALL_SECRETS})"
        for job in forwards_every_secret_elsewhere(document)
    )
    return offenders
