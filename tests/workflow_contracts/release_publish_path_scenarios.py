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
    failed_build: str | None = None,
    failed_need: str | None = None,
    skipped_need: str | None = None,
) -> dict[str, str]:
    """Build a complete needs-result map with one selected outcome."""
    results = dict.fromkeys(CRATES_IO_NEEDS, "success")
    results["windows-native-recipe-smoke"] = smoke
    if failed_build is not None:
        results[failed_build] = "failure"
    if failed_need is not None:
        results[failed_need] = "failure"
    if skipped_need is not None:
        results[skipped_need] = "skipped"
    return results


def _prerequisite_scenarios(outcome: str) -> tuple[Scenario, ...]:
    """Create one publishing scenario for each failed or skipped prerequisite."""
    return tuple(
        Scenario(
            f"publish-{need}-{outcome}",
            "publish",
            "",
            _needs_results(
                failed_need=need if outcome == "failure" else None,
                skipped_need=need if outcome == "skipped" else None,
            ),
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
        "dry-run-failed-build",
        "dry-run",
        "opened",
        _needs_results(smoke="skipped", failed_build="build-linux"),
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
        _needs_results(failed_build="build-linux"),
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
