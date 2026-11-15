"""Build labelled workflow mutations for the release-path contract."""

import copy
import dataclasses
import enum
import re
import typing as typ

from release_publish_crates_io_mutations import CRATES_IO_MUTATIONS
from release_publish_path_artifacts import BUILD_JOBS
from release_publish_path_authorization import AUTHORIZATION_STEP, RESOLVER_STEP
from workflow_loading import (
    job_steps,
    named_step,
    require_mapping,
    step_index_by_key,
    workflow_job,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

EXPECTED_VIOLATIONS = {
    "publish-only-job-guard": tuple(
        (scenario, "release-job")
        for scenario in ("opened", "synchronize", "reopened", "ready-for-review")
    ),
    "publish-guard-download": (("opened", "download"),),
    "publish-guard-hoist": (("opened", "hoist"),),
    "publish-guard-upload": (("opened", "upload"),),
    "remove-upload-dry-run": (("opened", "upload-plan"),),
    "falsify-upload-dry-run": (("opened", "upload-plan"),),
    "remove-draft-guard": (("opened", "draft"),),
    "remove-cancelled-guard": (("cancelled-ready-for-review", "release-job"),),
    "remove-caller-event": (("caller", "pull-request-types"),),
    "remove-skipped-smoke-tolerance": (("opened", "release-job"),),
    "allow-skipped-smoke-on-publish": (("tag-publish-skipped-smoke", "publish-job"),),
    "use-diagnostics-upload-mode": tuple(
        item
        for job, platform in BUILD_JOBS.items()
        for item in (
            ("opened", f"package-upload.{job}"),
            ("opened", f"platform-upload.{job}.{platform}"),
        )
    ),
    "remove-crates-io-oidc-permission": (("static", "publish-crates-io.permissions"),),
    "grant-crates-io-contents-write": (("static", "publish-crates-io.permissions"),),
    "remove-crates-io-environment": (("static", "publish-crates-io.environment"),),
    "drop-crate-package-prerequisite": (("static", "publish-crates-io.needs"),),
    "select-unrelated-crate-artifact": (("static", "publish-crates-io.artifact"),),
    "authenticate-before-classify": (("static", "publish-crates-io.action-order"),),
    "authenticate-without-publish-guard": (("static", "publish-crates-io.auth-guard"),),
    "rebuild-crate-before-upload": (("static", "publish-crates-io.action.rebuild"),),
    "metadata-reads-resolver": (
        ("reusable-push-publish-false", "metadata.should_publish"),
    ),
    "remove-authorization-step": (("static", "authorization.step"),),
    "validate-ref-reads-resolver": (("static", "authorization.validate-ref-gate"),),
    "version-tag-reads-resolver": (("static", "authorization.version-tag-gate"),),
    "authorization-ignores-workflow-ref": (
        ("static", "authorization.env.INPUT_WORKFLOW_REF"),
    ),
    "remove-dry-run-veto": (("static", "publish-release.dry-run-veto"),),
}
MUTATION_LABELS = tuple(EXPECTED_VIOLATIONS)


@dataclasses.dataclass(frozen=True, slots=True)
class MutationCase:
    """Hold one labelled mutation and its required scenario violations."""

    label: str
    release: dict[str, object]
    caller: dict[str, object]
    build: dict[str, object]
    action: dict[str, object]
    expected_violations: tuple[tuple[str, str], ...]


@dataclasses.dataclass(frozen=True, slots=True)
class WorkflowInputs:
    """Group the parsed documents needed to build a mutation case."""

    release: dict[str, object]
    caller: dict[str, object]
    build: dict[str, object]
    action: dict[str, object]


class MutationIssue(enum.StrEnum):
    """Classify a stale mutation definition or unknown mutation label."""

    PATTERN_MISMATCH = "mutation pattern did not match exactly once"
    UNKNOWN_LABEL = "unknown release-path mutation"
    MISSING_CALLER_EVENT = "caller pull_request.types must include synchronize"


class MutationError(AssertionError):
    """Describe a mutation helper that no longer matches its test fixture."""

    def __init__(self, issue: MutationIssue, detail: object | None = None) -> None:
        """Record the mutation error category and optional detail."""
        self.issue = issue
        self.detail = detail
        super().__init__(issue, detail)

    def __str__(self) -> str:
        """Render the mutation error with any stale pattern or label."""
        if self.detail is None:
            return self.issue.value
        return f"{self.issue.value}: {self.detail}"


def _replace_once(value: str, pattern: str, replacement: str) -> str:
    """Replace one required expression shape, refusing stale mutation rules."""
    mutated, count = re.subn(pattern, replacement, value, count=1)
    if count != 1:
        raise MutationError(MutationIssue.PATTERN_MISMATCH, (pattern, count))
    return mutated


def _release_steps(release: dict[str, object]) -> list[dict[str, object]]:
    """Return steps from the copied release job."""
    return job_steps(release, "release")


def _mutate_release_job_guard(release: dict[str, object]) -> None:
    """Put the release job back behind publication mode alone."""
    workflow_job(release, "release")["if"] = (
        "needs.metadata.outputs.should_publish == 'true'"
    )


def _mutate_staging_guard(release: dict[str, object], label: str) -> None:
    """Put one staging step back behind publication mode."""
    steps = _release_steps(release)
    if label.endswith("download"):
        step = steps[step_index_by_key(steps, "uses", "actions/download-artifact@")]
    elif label.endswith("hoist"):
        step = named_step(steps, "Hoist cargo-binstall archives to the release root")
    else:
        step = steps[step_index_by_key(steps, "id", "upload_assets")]
    step["if"] = "needs.metadata.outputs.should_publish == 'true'"


def _mutate_upload_dry_run(release: dict[str, object], *, falsify: bool) -> None:
    """Remove or disable plan mode on the release asset action."""
    upload = named_step(_release_steps(release), "Upload artefacts to release")
    with_values = require_mapping(upload.get("with"), "upload-release-assets.with")
    if falsify:
        with_values["dry-run"] = False
    else:
        with_values.pop("dry-run", None)


def _mutate_draft_guard(release: dict[str, object]) -> None:
    """Remove the publish-only condition from draft creation."""
    draft = named_step(
        job_steps(release, "publish-release"), "Ensure release exists (draft)"
    )
    draft.pop("if", None)


def _mutate_cancelled_guard(release: dict[str, object]) -> None:
    """Remove the explicit cancellation check from the release job."""
    job = workflow_job(release, "release")
    job["if"] = _replace_once(str(job.get("if", "")), r"!cancelled\(\)\s*&&\s*", "")


def _mutate_smoke_tolerance(release: dict[str, object], *, allow_publish: bool) -> None:
    """Remove skipped-smoke tolerance or make it apply to publication too."""
    job_name = "publish-release" if allow_publish else "release"
    job = workflow_job(release, job_name)
    guard = str(job.get("if", ""))
    if allow_publish:
        pattern = r"needs\.windows-native-recipe-smoke\.result == 'success'"
        replacement = (
            "(needs.windows-native-recipe-smoke.result == 'success' || "
            "needs.windows-native-recipe-smoke.result == 'skipped')"
        )
    else:
        pattern = (
            r"needs\.windows-native-recipe-smoke\.result == 'success'\s*\|\|\s*"
            r"needs\.windows-native-recipe-smoke\.result == 'skipped'"
        )
        replacement = "needs.windows-native-recipe-smoke.result == 'success'"
    job["if"] = _replace_once(guard, pattern, replacement)


def _mutate_package_upload_source(release: dict[str, object]) -> None:
    """Reconnect package workflow uploads to the diagnostics upload mode."""
    for job_name in BUILD_JOBS:
        with_values = require_mapping(
            workflow_job(release, job_name).get("with"), f"{job_name}.with"
        )
        with_values["should-upload-workflow-artifacts"] = (
            "${{ fromJSON(needs.metadata.outputs.should_upload_workflow_artifacts) }}"
        )


def _metadata_reads_resolver(release: dict[str, object]) -> None:
    """Point every metadata output back at the resolver's own answer."""
    outputs = require_mapping(
        workflow_job(release, "metadata").get("outputs"), "metadata.outputs"
    )
    replaced = 0
    for name, expression in outputs.items():
        if isinstance(expression, str) and AUTHORIZATION_STEP in expression:
            outputs[name] = expression.replace(
                f"steps.{AUTHORIZATION_STEP}.outputs",
                f"steps.{RESOLVER_STEP}.outputs",
            )
            replaced += 1
    if replaced == 0:
        raise MutationError(MutationIssue.PATTERN_MISMATCH, ("metadata.outputs", 0))


def _remove_authorization_step(release: dict[str, object]) -> None:
    """Delete the step that decides release writes."""
    steps = job_steps(release, "metadata")
    steps.pop(step_index_by_key(steps, "id", AUTHORIZATION_STEP))


def _validate_ref_reads_resolver(release: dict[str, object]) -> None:
    """Let the ref gate follow the resolver instead of the decision."""
    steps = job_steps(release, "metadata")
    validate = steps[step_index_by_key(steps, "id", "validate_publish_ref")]
    validate["if"] = f"steps.{RESOLVER_STEP}.outputs['should-publish'] == 'true'"


def _version_tag_reads_resolver(release: dict[str, object]) -> None:
    """Let version checking follow the resolver instead of the decision."""
    steps = job_steps(release, "metadata")
    version = steps[step_index_by_key(steps, "id", "ensure_version")]
    with_values = require_mapping(version.get("with"), "ensure_version.with")
    with_values["check-tag"] = (
        f"${{{{ fromJSON(steps.{RESOLVER_STEP}.outputs['should-publish']) }}}}"
    )


def _authorization_ignores_workflow_ref(release: dict[str, object]) -> None:
    """Feed the policy the triggering ref in place of the workflow ref."""
    steps = job_steps(release, "metadata")
    step = steps[step_index_by_key(steps, "id", AUTHORIZATION_STEP)]
    env = require_mapping(step.get("env"), "authorization.env")
    env["INPUT_WORKFLOW_REF"] = "${{ github.ref }}"


def _remove_dry_run_veto(release: dict[str, object]) -> None:
    """Drop the dry-run veto from the write job's guard."""
    job = workflow_job(release, "publish-release")
    job["if"] = _replace_once(
        str(job.get("if", "")),
        r"needs\.metadata\.outputs\.dry_run != 'true'\s*&&\s*",
        "",
    )


def mutate_workflows(
    label: str,
    workflows: WorkflowInputs,
) -> MutationCase:
    """Deep-copy parsed workflows and apply one named reachability mutation.

    Parameters
    ----------
    label : str
        Name of the registered mutation to apply.
    workflows : WorkflowInputs
        Parsed workflows to copy before applying the mutation.

    Returns
    -------
    MutationCase
        Mutated workflow documents and the violations expected from them.

    Raises
    ------
    MutationError
        If ``label`` does not name a registered reachability mutation.
    """
    mutant_release = copy.deepcopy(workflows.release)
    mutant_caller = copy.deepcopy(workflows.caller)
    mutant_build = copy.deepcopy(workflows.build)
    mutant_action = copy.deepcopy(workflows.action)
    applier = _MUTATION_APPLIERS.get(label)
    if applier is not None:
        applier(mutant_release, mutant_caller)
    elif label in CRATES_IO_MUTATIONS:
        CRATES_IO_MUTATIONS[label](mutant_release, mutant_action)
    else:
        raise MutationError(MutationIssue.UNKNOWN_LABEL, label)
    return MutationCase(
        label,
        mutant_release,
        mutant_caller,
        mutant_build,
        mutant_action,
        EXPECTED_VIOLATIONS[label],
    )


def _mutate_caller_event(caller: dict[str, object]) -> None:
    """Remove one dry-run activity type from the caller workflow."""
    triggers = require_mapping(caller.get("on", caller.get(True)), "caller triggers")
    pull_request = require_mapping(triggers.get("pull_request"), "caller pull_request")
    event_types = pull_request.get("types")
    if not isinstance(event_types, list) or "synchronize" not in event_types:
        raise MutationError(MutationIssue.MISSING_CALLER_EVENT)
    event_types.remove("synchronize")


_MUTATION_APPLIERS: dict[
    str, cabc.Callable[[dict[str, object], dict[str, object]], None]
] = {
    "publish-only-job-guard": lambda release, caller: _mutate_release_job_guard(
        release
    ),
    "publish-guard-download": lambda release, caller: _mutate_staging_guard(
        release, "publish-guard-download"
    ),
    "publish-guard-hoist": lambda release, caller: _mutate_staging_guard(
        release, "publish-guard-hoist"
    ),
    "publish-guard-upload": lambda release, caller: _mutate_staging_guard(
        release, "publish-guard-upload"
    ),
    "remove-upload-dry-run": lambda release, caller: _mutate_upload_dry_run(
        release, falsify=False
    ),
    "falsify-upload-dry-run": lambda release, caller: _mutate_upload_dry_run(
        release, falsify=True
    ),
    "remove-draft-guard": lambda release, caller: _mutate_draft_guard(release),
    "remove-cancelled-guard": lambda release, caller: _mutate_cancelled_guard(release),
    "remove-skipped-smoke-tolerance": lambda release, caller: _mutate_smoke_tolerance(
        release, allow_publish=False
    ),
    "allow-skipped-smoke-on-publish": lambda release, caller: _mutate_smoke_tolerance(
        release, allow_publish=True
    ),
    "use-diagnostics-upload-mode": lambda release, caller: (
        _mutate_package_upload_source(release)
    ),
    "remove-caller-event": lambda release, caller: _mutate_caller_event(caller),
    "metadata-reads-resolver": lambda release, caller: _metadata_reads_resolver(
        release
    ),
    "remove-authorization-step": lambda release, caller: _remove_authorization_step(
        release
    ),
    "validate-ref-reads-resolver": lambda release, caller: _validate_ref_reads_resolver(
        release
    ),
    "version-tag-reads-resolver": lambda release, caller: _version_tag_reads_resolver(
        release
    ),
    "authorization-ignores-workflow-ref": lambda release, caller: (
        _authorization_ignores_workflow_ref(release)
    ),
    "remove-dry-run-veto": lambda release, caller: _remove_dry_run_veto(release),
}
