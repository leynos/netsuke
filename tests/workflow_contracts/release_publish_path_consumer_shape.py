"""Hold the consumer-smoke job and action to their shape and privilege.

The smoke is the only release job that exercises the release the way a
consumer does, and the only one that must hold no credential at all. Its
contract is therefore as much about what it does *not* have — no token input,
no persisted checkout, no write permission, no deployment environment, no Rust
toolchain — as about the download it performs. Each property here is asserted
against the checked-in workflow and action, so a later edit that reaches for a
credential or a compiler fails a contract instead of quietly passing.

The credential scans read the job and the action together and live in
``release_publish_path_consumer_credentials``, because a token can be defined in
either document. The scenario-level composition and the reachability oracle
live in ``release_publish_path_consumer_smoke``, which re-exports everything
these modules define.

Run via ``make test-workflow-contracts``.
"""

import re

from release_publish_needs import job_names
from release_publish_path_scenarios import CONSUMER_SMOKE_NEEDS

CONSUMER_SMOKE_JOB = "consumer-smoke"
#: The action the job must call, and the manifest that action must be.
LOCAL_SMOKE_ACTION = "./.github/actions/consumer-smoke"
CONSUMER_SMOKE_ACTION_PATH = ".github/actions/consumer-smoke/action.yml"
#: The whole grant. The smoke reads public bytes and downloads them; it never
#: writes repository state, exchanges an OIDC token, or enters an environment.
CONSUMER_SMOKE_PERMISSIONS = {"contents": "read"}
#: The only artefact names the job may download. The package record is the
#: candidate's evidence and the trusted record is the publisher's; both arrive
#: under fixed names. Anything else — above all a ``pattern`` — would let
#: workflow artefact storage stand in for the public download the smoke exists
#: to exercise.
PACKAGE_RECORD_ARTIFACT = "release-crate-package"
SMOKE_ARTIFACTS = frozenset({
    PACKAGE_RECORD_ARTIFACT,
    "${{ needs.finalize-github-release.outputs.trusted-artifact }}",
})
DOWNLOAD_ARTIFACT_PREFIX = "actions/download-artifact@"
#: The checkout the job must use, and the two values it must pass. A persisted
#: token would leave a credential in the job's git configuration, which is the
#: one credential a read-only job must not carry, and a checkout of the tag
#: would exercise whatever the tag resolves to rather than the candidate.
CHECKOUT_PREFIX = "actions/checkout@"
CANDIDATE_REF = "${{ needs.metadata.outputs.candidate_commit }}"
#: The target the shipped archive is executed on, and the exact inputs the job
#: must pass. The action is not a matrix: a second platform would need its own
#: runner rather than another row on this one.
SMOKE_ACTION_TARGET = "x86_64-unknown-linux-gnu"
SMOKE_ACTION_INPUTS = {
    "identity-path": f"{PACKAGE_RECORD_ARTIFACT}/artifacts/package/identity.json",
    "asset-digest-record": "trusted/trusted.json",
    "repository": "${{ github.repository }}",
    "commit": "${{ needs.metadata.outputs.candidate_commit }}",
    "target": SMOKE_ACTION_TARGET,
    "work-dir": "${{ runner.temp }}/release-consumer-smoke",
}
#: A whole-word Rust build command. A comment may name the archive format, so
#: only executable lines are read; a command line naming any of these would let
#: the smoke build what it is supposed to download.
COMPILATION_COMMAND = re.compile(r"\b(?:cargo|rustc|rustup)\b")
#: The expressions that resolve to a credential, whatever name stores them.
TOKEN_EXPRESSION = re.compile(r"\bgithub\.token\b|\bsecrets\b")
TOKEN_NAMES = frozenset({"GH_TOKEN", "GITHUB_TOKEN"})


def _command_lines(run: object) -> list[str]:
    """Return a step's executable shell lines, comments and continuations gone.

    A comment naming the cargo-binstall archive is documentation, not a build,
    so a check for a compiler must read what bash runs. Bash also deletes every
    backslash-newline before parsing, so an invocation split across lines is
    one command.

    Returns
    -------
    list[str]
        The stripped command lines, with comment lines removed.
    """
    if not isinstance(run, str):
        return []
    joined = run.replace("\\\n", "")
    return [
        line.strip()
        for line in joined.splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]


def steps_of(job: dict[str, object]) -> list[dict[str, object]]:
    """Return one job's steps, tolerating a job that declares none.

    A mutated job without steps must still produce a contract violation rather
    than an exception from the reader.

    Returns
    -------
    list[dict[str, object]]
        Each step mapping, in declaration order, or an empty list.
    """
    match job.get("steps"):
        case list() as steps:
            return [step for step in steps if isinstance(step, dict)]
        case _:
            return []


def action_steps(action: dict[str, object]) -> list[dict[str, object]]:
    """Return a composite action's steps, tolerating a manifest without any.

    Returns
    -------
    list[dict[str, object]]
        Each step mapping, in declaration order, or an empty list.
    """
    runs = action.get("runs")
    steps = runs.get("steps") if isinstance(runs, dict) else None
    match steps:
        case list() as declared:
            return [step for step in declared if isinstance(step, dict)]
        case _:
            return []


def smoke_job(release: dict[str, object], violations: list[str]) -> dict[str, object]:
    """Return the smoke job, or an empty job that every check then refuses.

    Returning an empty mapping keeps a workflow with no smoke job to one
    violation per missing property instead of an exception from the reader.

    Returns
    -------
    dict[str, object]
        The parsed job, or ``{}`` when the workflow declares none.
    """
    jobs = release.get("jobs")
    job = jobs.get(CONSUMER_SMOKE_JOB) if isinstance(jobs, dict) else None
    if isinstance(job, dict):
        return job
    violations.append(f"{CONSUMER_SMOKE_JOB}: the workflow declares no such job")
    return {}


def check_smoke_needs(job: dict[str, object], violations: list[str]) -> None:
    """Require exactly the candidate's three stages, with nothing else.

    The length comparison catches a repeated name, which set equality alone
    would accept and which GitHub treats as an error.
    """
    names = job_names(job.get("needs", []))
    if names is None:
        violations.append(f"{CONSUMER_SMOKE_JOB}.needs: dependencies must be jobs")
        return
    if set(names) != CONSUMER_SMOKE_NEEDS or len(names) != len(CONSUMER_SMOKE_NEEDS):
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.needs: expected exactly metadata, package-crate "
            "and finalize-github-release"
        )


def check_smoke_checkout(job: dict[str, object], violations: list[str]) -> None:
    """Require the candidate checkout to keep no credential in the job.

    ``actions/checkout`` writes the workflow token into the local git config
    by default, where every later step of the job can read it. The smoke reads
    public bytes and runs one executable, so the token would be a credential
    held for no reason; and checking out the tag rather than the resolved
    candidate would exercise a tree the publisher never built.
    """
    checkouts = [
        step
        for step in steps_of(job)
        if str(step.get("uses", "")).startswith(CHECKOUT_PREFIX)
    ]
    if len(checkouts) != 1:
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.checkout: expected exactly one checkout "
            f"step, found {len(checkouts)}"
        )
        return
    values = checkouts[0].get("with")
    if not isinstance(values, dict):
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.checkout: the checkout must pass "
            "persist-credentials: false and the candidate commit"
        )
        return
    _require_checkout_values(values, violations)


def _require_checkout_values(
    values: dict[str, object], violations: list[str]
) -> None:
    """Require the checkout's inputs to drop the token and pin the candidate."""
    # `is not False` rather than a falsy test: an omitted key, the string
    # "false", and a `true` all leave a credential, and only a YAML boolean
    # false removes it.
    if values.get("persist-credentials") is not False:
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.checkout: persist-credentials must be "
            f"false, got {values.get('persist-credentials')!r}"
        )
    if values.get("ref") != CANDIDATE_REF:
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.checkout: the checkout must pin the "
            f"candidate commit, got {values.get('ref')!r}"
        )


def check_smoke_artifacts(job: dict[str, object], violations: list[str]) -> None:
    """Require every artefact download to name one permitted artefact exactly."""
    for step in steps_of(job):
        if str(step.get("uses", "")).startswith(DOWNLOAD_ARTIFACT_PREFIX):
            _require_named_artifact(step, violations)


def _require_named_artifact(
    step: dict[str, object], violations: list[str]
) -> None:
    """Require one artefact download to name its artefact and no pattern."""
    values = step.get("with")
    if not isinstance(values, dict):
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.artifacts: every download must name its artefact"
        )
        return
    if "pattern" in values:
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.artifacts: a pattern download would let "
            "artefact storage stand in for the public download"
        )
    if str(values.get("name")) not in SMOKE_ARTIFACTS:
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.artifacts: {values.get('name')!r} is not "
            "one of the permitted artefact names"
        )


def check_smoke_action_inputs(job: dict[str, object], violations: list[str]) -> None:
    """Require the one local-action invocation, its inputs, and no step condition.

    A step-level ``if`` is refused because it can stop the smoke without
    touching anything else: the job guard, the dependency list, and the inputs
    would all still read as correct while the one step that exercises the
    release never ran. The job's own guard is the only condition allowed to
    decide whether the smoke runs.
    """
    invocations = [
        step
        for step in steps_of(job)
        if str(step.get("uses", "")) == LOCAL_SMOKE_ACTION
    ]
    if len(invocations) != 1:
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.action: expected exactly one "
            f"{LOCAL_SMOKE_ACTION} step, found {len(invocations)}"
        )
        return
    _require_invocation(invocations[0], violations)


def _require_invocation(step: dict[str, object], violations: list[str]) -> None:
    """Require the invocation to carry no condition and exactly the reviewed inputs."""
    if "if" in step:
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.action: the smoke invocation must run "
            f"whenever the job does, got if: {step['if']!r}"
        )
    values = step.get("with")
    if values != SMOKE_ACTION_INPUTS:
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.action: inputs must be exactly "
            f"{sorted(SMOKE_ACTION_INPUTS)}, got {values!r}"
        )


def check_smoke_action_declared(
    action: dict[str, object] | None, violations: list[str]
) -> None:
    """Require the action to declare the six reviewed inputs and no credential."""
    if action is None:
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.action: {CONSUMER_SMOKE_ACTION_PATH} is missing"
        )
        return
    declared = action.get("inputs")
    names = set(declared) if isinstance(declared, dict) else set()
    if names != set(SMOKE_ACTION_INPUTS):
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.action: inputs must be exactly "
            f"{sorted(SMOKE_ACTION_INPUTS)}, got {sorted(names)}"
        )
    credential = _credential_inputs(names)
    if credential:
        violations.append(
            f"{CONSUMER_SMOKE_JOB}.action: the smoke must declare no credential "
            f"input, got {credential}"
        )


def _credential_inputs(names: set[str]) -> list[str]:
    """Return the declared input names that resolve to a credential.

    Returns
    -------
    list[str]
        The offending names, sorted, or an empty list. A name is judged by the
        expression it holds and by the word it uses, because a token can be
        spelled into an input either way.
    """
    return sorted(
        name for name in names if TOKEN_EXPRESSION.search(name) or "token" in name
    )


def check_smoke_compilation(
    job: dict[str, object],
    action: dict[str, object] | None,
    violations: list[str],
) -> None:
    """Refuse a build command anywhere in the job or the action it calls.

    The smoke exists to run the bytes a consumer receives. A compiler here
    would replace the download with a build, and ``cargo install netsuke-build``
    is the specific substitution this check was written for.
    """
    sources: list[tuple[str, object]] = [
        (f"{CONSUMER_SMOKE_JOB} step {index}", step.get("run"))
        for index, step in enumerate(steps_of(job))
    ]
    if action is not None:
        sources.extend(
            (f"{CONSUMER_SMOKE_ACTION_PATH} step {index}", step.get("run"))
            for index, step in enumerate(action_steps(action))
        )
    for label, run in sources:
        offending = [
            line for line in _command_lines(run) if COMPILATION_COMMAND.search(line)
        ]
        if offending:
            violations.append(
                f"{CONSUMER_SMOKE_JOB}.compilation: {label} builds instead of "
                f"downloading, got {offending[0]!r}"
            )


__all__ = [
    "CANDIDATE_REF",
    "CHECKOUT_PREFIX",
    "CONSUMER_SMOKE_ACTION_PATH",
    "CONSUMER_SMOKE_JOB",
    "CONSUMER_SMOKE_NEEDS",
    "CONSUMER_SMOKE_PERMISSIONS",
    "LOCAL_SMOKE_ACTION",
    "PACKAGE_RECORD_ARTIFACT",
    "SMOKE_ACTION_INPUTS",
    "SMOKE_ACTION_TARGET",
    "SMOKE_ARTIFACTS",
    "action_steps",
    "check_smoke_action_declared",
    "check_smoke_action_inputs",
    "check_smoke_artifacts",
    "check_smoke_checkout",
    "check_smoke_compilation",
    "check_smoke_needs",
    "smoke_job",
    "steps_of",
]
