"""Model the finite release modes and dependency outcomes in workflow tests."""

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


@dataclasses.dataclass(frozen=True, slots=True)
class Scenario:
    """Describe the external release mode and dependency outcomes for one run."""

    name: str
    mode: str
    event_action: str
    needs_results: cabc.Mapping[str, str]
    cancelled: bool
    release_runs: bool


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
    """
    results = dict.fromkeys(CRATES_IO_NEEDS | PUBLISHER_NEEDS, "success")
    results["windows-native-recipe-smoke"] = smoke
    if outcomes is not None:
        results.update(outcomes)
    return results


def _prerequisite_scenarios(outcome: str) -> tuple[Scenario, ...]:
    """Create one publishing scenario for each failed or skipped prerequisite."""
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


SCENARIOS = (
    Scenario(
        "opened",
        "dry-run",
        "opened",
        _needs_results(smoke="skipped"),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        "synchronize",
        "dry-run",
        "synchronize",
        _needs_results(smoke="skipped"),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        "reopened",
        "dry-run",
        "reopened",
        _needs_results(smoke="skipped"),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        "ready-for-review",
        "dry-run",
        "ready_for_review",
        _needs_results(),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        # The realistic dry run: neither publisher runs, so the verification
        # job must be skipped even though the release staging job runs.
        "dry-run-publishers-skipped",
        "dry-run",
        "opened",
        _needs_results(
            smoke="skipped",
            outcomes={"publish-crates-io": "skipped", "publish-release": "skipped"},
        ),
        cancelled=False,
        release_runs=True,
    ),
    Scenario(
        "dry-run-failed-build",
        "dry-run",
        "opened",
        _needs_results(smoke="skipped", outcomes={"build-linux": "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "tag-publish",
        "publish",
        "",
        _needs_results(),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "tag-publish-skipped-smoke",
        "publish",
        "",
        _needs_results(smoke="skipped"),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "tag-publish-failed-smoke",
        "publish",
        "",
        _needs_results(smoke="failure"),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "tag-publish-failed-build",
        "publish",
        "",
        _needs_results(outcomes={"build-linux": "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "publish-crates-io-failure",
        "publish",
        "",
        _needs_results(outcomes={"publish-crates-io": "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "publish-crates-io-skipped",
        "publish",
        "",
        _needs_results(outcomes={"publish-crates-io": "skipped"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "publish-release-failure",
        "publish",
        "",
        _needs_results(outcomes={"publish-release": "failure"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "publish-release-skipped",
        "publish",
        "",
        _needs_results(outcomes={"publish-release": "skipped"}),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "neither-mode",
        "neither",
        "",
        _needs_results(),
        cancelled=False,
        release_runs=False,
    ),
    Scenario(
        "cancelled-ready-for-review",
        "dry-run",
        "ready_for_review",
        _needs_results(),
        cancelled=True,
        release_runs=False,
    ),
    Scenario(
        "cancelled-publishing",
        "publish",
        "",
        _needs_results(),
        cancelled=True,
        release_runs=False,
    ),
    *_prerequisite_scenarios("failure"),
    *_prerequisite_scenarios("skipped"),
)
