"""Model one release invocation and the decision the workflow must make.

Each scenario pairs the invocation a run arrived from with the pinned
``determine-release-modes`` answer for its trigger. The workflow's own decision
is then the real ``scripts/authorize_release_writes.py`` policy, evaluated over
that pair, so the contract checks the wiring against the same rule a live run
applies rather than a second copy of it.

The resolver answers ``should-publish: true`` for every ``push``, so the
reusable-push scenarios are the security cases: a caller that inherited a push
trigger must still refuse to publish unless it asked to. The table itself is in
``release_publish_path_scenarios``; this module holds only the value objects and
the resolvers they share.
"""

import dataclasses
import typing as typ

import authorize_release_writes as adapter
import release_write_policy as policy

if typ.TYPE_CHECKING:
    import collections.abc as cabc

REPOSITORY = "leynos/netsuke"
TAG_REF = "refs/tags/v1.2.3"
BRANCH_REF = "refs/heads/main"
PULL_REF = "refs/pull/1/merge"
DIRECT_WORKFLOW_REF = f"{REPOSITORY}/.github/workflows/release.yml@{TAG_REF}"
CALLER_WORKFLOW_REF = f"{REPOSITORY}/.github/workflows/release-dry-run.yml@{BRANCH_REF}"

#: The resolver's answer per release mode. ``mode`` names the resolver's view,
#: which the policy may narrow but must never widen.
MODE_OUTPUTS = {
    "dry-run": {
        "should-publish": "false",
        "dry-run": "true",
        "should-upload-workflow-artifacts": "false",
    },
    "publish": {
        "should-publish": "true",
        "dry-run": "false",
        "should-upload-workflow-artifacts": "true",
    },
    "neither": {
        "should-publish": "false",
        "dry-run": "false",
        "should-upload-workflow-artifacts": "false",
    },
}
#: A resolver that selected publication and a dry run at once, which the policy
#: must read as a veto rather than as permission.
PUBLISHING_DRY_RUN = {
    "should-publish": "true",
    "dry-run": "true",
    "should-upload-workflow-artifacts": "false",
}
REQUIRED_DRY_RUN_EVENTS = {"opened", "synchronize", "reopened", "ready_for_review"}
REQUIRED_NEEDS = {
    "metadata",
    "build-linux",
    "build-windows",
    "build-macos",
    "windows-native-recipe-smoke",
}
CRATES_IO_NEEDS = {
    "metadata",
    "package-crate",
    "build-linux",
    "build-windows",
    "build-macos",
    "windows-native-recipe-smoke",
    "release-admission-canaries",
}
PUBLISHER_NEEDS = {"publish-crates-io", "publish-release"}
#: The verification job depends on both publishers and on nothing else, so a
#: failed or skipped publisher leaves the inventory unverified rather than
#: verified against a release that was never completed.
VERIFY_NEEDS = {"metadata", *PUBLISHER_NEEDS}
#: The consumer smoke depends on the candidate's three stages and on nothing
#: else. Finalization is one of them because an archive attached to a draft
#: release needs a token to download, and the smoke deliberately holds none.
CONSUMER_SMOKE_NEEDS = {"metadata", "package-crate", "finalize-github-release"}


@dataclasses.dataclass(frozen=True, slots=True)
class Invocation:
    """Describe how a run reached the release workflow.

    ``publish`` and ``dry_run`` are the reusable inputs as GitHub materializes
    them: empty on a direct trigger, otherwise ``"true"`` or ``"false"``.
    """

    publish: str
    dry_run: str
    event_name: str
    ref: str
    workflow_ref: str
    repository: str = REPOSITORY


DIRECT_PUSH = Invocation("", "", "push", TAG_REF, DIRECT_WORKFLOW_REF)
DRY_RUN_CALL = Invocation(
    "false", "true", "pull_request", PULL_REF, CALLER_WORKFLOW_REF
)
OTHER_EVENT_CALL = Invocation(
    "false", "false", "workflow_dispatch", BRANCH_REF, CALLER_WORKFLOW_REF
)
#: The invocation each release mode assumes when a scenario does not name one.
MODE_INVOCATIONS = {
    "publish": DIRECT_PUSH,
    "dry-run": DRY_RUN_CALL,
    "neither": OTHER_EVENT_CALL,
}


@dataclasses.dataclass(frozen=True, slots=True)
class Scenario:
    """Describe one release invocation and its dependency outcomes."""

    name: str
    mode: str
    event_action: str
    needs_results: cabc.Mapping[str, str]
    cancelled: bool
    release_runs: bool
    invocation: Invocation | None = None
    resolver: cabc.Mapping[str, str] | None = None

    @property
    def arrival(self) -> Invocation:
        """How the run reached the workflow."""
        return (
            self.invocation
            if self.invocation is not None
            else (MODE_INVOCATIONS[self.mode])
        )

    @property
    def resolver_outputs(self) -> cabc.Mapping[str, str]:
        """The resolver answer this scenario models."""
        return self.resolver if self.resolver is not None else MODE_OUTPUTS[self.mode]

    def policy_context(self) -> policy.ReleaseContext:
        """Build the pure policy context for this scenario.

        The raw values are handed to the same adapter the workflow step runs, so
        the contract exercises the parsing and origin classification a live run
        performs instead of reproducing them.

        Returns
        -------
        policy.ReleaseContext
            The invocation and resolver answers the policy decides over.
        """
        arrival = self.arrival
        resolver = self.resolver_outputs
        return adapter.WorkflowInputs(
            publish=arrival.publish,
            dry_run=arrival.dry_run,
            event_name=arrival.event_name,
            ref=arrival.ref,
            workflow_ref=arrival.workflow_ref,
            repository=arrival.repository,
            resolver_should_publish=resolver["should-publish"],
            resolver_dry_run=resolver["dry-run"],
            resolver_should_upload_workflow_artifacts=resolver[
                "should-upload-workflow-artifacts"
            ],
        ).as_context()

    @property
    def authorization(self) -> policy.ReleaseAuthorization:
        """The policy's decision for this scenario."""
        return policy.authorize_release_writes(self.policy_context())

    @property
    def outputs(self) -> dict[str, str]:
        """The metadata outputs a correctly wired workflow must publish."""
        decision = self.authorization
        upload = decision.should_upload_workflow_artifacts
        return {
            "should_publish": _render(flag=decision.should_publish),
            "dry_run": _render(flag=decision.dry_run),
            "should_upload_workflow_artifacts": _render(flag=upload),
            # Staging rehearsal also needs the package artefacts, so the
            # workflow admits them on a dry run as well as on a diagnostics run.
            "should_upload_package_artifacts": _render(flag=upload or decision.dry_run),
        }


def _render(*, flag: bool) -> str:
    """Render one decision flag as the workflow output spelling."""
    return "true" if flag else "false"


def needs_results(
    *,
    smoke: str = "success",
    outcomes: cabc.Mapping[str, str] | None = None,
) -> dict[str, str]:
    """Build a complete needs-result map from the selected outcomes.

    Every dependency defaults to success, so a scenario names only the ones it
    changes. ``outcomes`` is applied last and therefore wins over ``smoke``,
    which keeps the smoke shortcut from silently overriding an explicit
    outcome for the same job.

    Returns
    -------
    dict[str, str]
        One result per dependency named by the release scenarios.

    Raises
    ------
    ValueError
        If ``outcomes`` names a job the release scenarios do not model. Its
        result would otherwise land beside the dependency the scenario meant to
        change, leaving every real dependency at success and the scenario green
        while testing nothing.

    Examples
    --------
    >>> needs_results(outcomes={"build-linux": "failure"})["build-linux"]
    'failure'
    >>> needs_results(smoke="skipped")["windows-native-recipe-smoke"]
    'skipped'
    """
    results = dict.fromkeys(
        CRATES_IO_NEEDS | PUBLISHER_NEEDS | CONSUMER_SMOKE_NEEDS, "success"
    )
    results["windows-native-recipe-smoke"] = smoke
    if outcomes is not None:
        # A mistyped job name would otherwise add a result no job declares and
        # leave the intended one at its default, so the scenario would assert
        # the opposite of what its name says.
        unknown = sorted(outcomes.keys() - results.keys())
        if unknown:
            msg = f"scenario names unknown dependenc(ies) {unknown}"
            raise ValueError(msg)
        results.update(outcomes)
    return results
