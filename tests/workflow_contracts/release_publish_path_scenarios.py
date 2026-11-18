"""Model the finite release modes and dependency outcomes in workflow tests.

A scenario names its mode, trigger event, each dependency's result, whether the
run was cancelled, and the gate outputs it would publish. This module supplies
the constants and helpers; ``release_publish_path_scenario_table`` lists the
scenarios themselves, and that table is the input to the reachability and
finalization checks.
"""

import dataclasses
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

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
PACKAGE_UPLOAD = {"dry-run": True, "publish": True, "neither": False}
REQUIRED_DRY_RUN_EVENTS = {"opened", "synchronize", "reopened", "ready_for_review"}
#: The commit every scenario releases. ``github.sha`` reports it and the gate
#: publishes it as ``candidate_commit``, so only an admitted candidate matches.
GITHUB_SHA = "a" * 40
#: Every prerequisite a publishing job waits on. Both publishers wait on the
#: same set by design -- a difference would let one lane publish bytes the
#: other refuses.
REQUIRED_NEEDS = {
    "metadata",
    "package-crate",
    "build-linux",
    "build-windows",
    "build-macos",
    "windows-native-recipe-smoke",
    "release-admission-canaries",
}
CRATES_IO_NEEDS = REQUIRED_NEEDS
#: The gate's outputs as a successful, candidate-matching run reports them.
#: Only ``enforcement`` differs: a rehearsal observes, a publishing run enforces.
ADMISSION_OUTPUTS = {
    mode: {
        "enforcement": "true" if mode == "publish" else "false",
        "gate_outcome": "success",
        "candidate_commit": GITHUB_SHA,
    }
    for mode in MODE_OUTPUTS
}
#: The rehearsal job's dependencies. Packaging and admission are absent by
#: design, so a rehearsal stays reachable when either fails.
STAGING_NEEDS = {
    "metadata",
    "build-linux",
    "build-windows",
    "build-macos",
    "windows-native-recipe-smoke",
}
#: The dependencies a rehearsal must never acquire, because either would let a
#: failure make the read-only lane unreachable.
STAGING_FORBIDDEN_NEEDS = {"package-crate", "release-admission-canaries"}
PUBLISHER_NEEDS = {"publish-crates-io", "publish-release"}
#: The two terminal write jobs. A scenario declares their own outcomes
#: directly, because no later job's result is derived from them.
VERIFY_JOB = "verify-release-inventory"
FINALIZE_JOB = "finalize-github-release"
#: Verification holds write scope, so it carries publication's gate: packaging,
#: enforcement, and both publishers, because it verifies what they produced. A
#: failed, skipped, or report-only prerequisite leaves it unverified.
VERIFY_NEEDS = {
    "metadata",
    "package-crate",
    "release-admission-canaries",
    *PUBLISHER_NEEDS,
}
#: The consumer smoke depends on the candidate's three stages and on nothing
#: else. Finalization is one of them because an archive attached to a draft
#: release needs a token to download, and the smoke deliberately holds none.
CONSUMER_SMOKE_NEEDS = {"metadata", "package-crate", "finalize-github-release"}


@dataclasses.dataclass(frozen=True, slots=True)
class Scenario:
    """Describe the external release mode and dependency outcomes for one run."""

    name: str
    mode: str
    event_action: str
    needs_results: cabc.Mapping[str, str]
    cancelled: bool
    release_runs: bool
    admission_outputs: dict[str, str] | None = None


def admission_outputs(scenario: Scenario) -> dict[str, str]:
    """Return the admission outputs the scenario's gate reports.

    Returns
    -------
    dict[str, str]
        The override when the scenario names one, else the mode's defaults.
    """
    if scenario.admission_outputs is not None:
        return dict(scenario.admission_outputs)
    return dict(ADMISSION_OUTPUTS[scenario.mode])


def admits_candidate(scenario: Scenario) -> bool:
    """Return whether the scenario's admission outputs admit its candidate."""
    outputs = admission_outputs(scenario)
    return (
        outputs.get("enforcement") == "true"
        and outputs.get("gate_outcome") == "success"
        and outputs.get("candidate_commit") == GITHUB_SHA
    )


def _needs_results(
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
    """
    results = dict.fromkeys(
        CRATES_IO_NEEDS
        | PUBLISHER_NEEDS
        | CONSUMER_SMOKE_NEEDS
        | {VERIFY_JOB, FINALIZE_JOB},
        "success",
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


def _prerequisite_scenarios(outcome: str) -> tuple[Scenario, ...]:
    """Create one publishing scenario per prerequisite carrying *outcome*."""
    return tuple(
        Scenario(
            f"publish-{need}-{outcome}",
            "publish",
            "",
            _needs_results(outcomes={need: outcome}),
            cancelled=False,
            release_runs=False,
        )
        for need in sorted(CRATES_IO_NEEDS)
    )
