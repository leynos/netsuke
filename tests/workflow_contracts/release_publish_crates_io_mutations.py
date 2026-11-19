"""Apply the crates.io publisher mutations the release contract expects.

The registry publisher is the only job that exchanges an OIDC token for a
short-lived registry credential, so its mutations are a kind of judgement of
their own: each one breaks exactly one of the permission, environment,
prerequisite, or ordering guarantees that keep the token confined to an
authorized publication.
"""

import typing as typ

from action_run_sources import action_steps
from release_publish_crates_io_contracts import ACTION_PATH
from workflow_loading import (
    job_steps,
    require_list,
    require_mapping,
    workflow_job,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc


def _remove_crates_io_oidc(
    release: dict[str, object], _action: dict[str, object]
) -> None:
    """Remove the registry job's OIDC token permission."""
    permissions = require_mapping(
        workflow_job(release, "publish-crates-io").get("permissions"),
        "publish-crates-io.permissions",
    )
    permissions.pop("id-token", None)


def _grant_crates_io_contents_write(
    release: dict[str, object], _action: dict[str, object]
) -> None:
    """Broaden the registry job's GitHub repository permission."""
    permissions = require_mapping(
        workflow_job(release, "publish-crates-io").get("permissions"),
        "publish-crates-io.permissions",
    )
    permissions["contents"] = "write"


def _remove_crates_io_environment(
    release: dict[str, object], _action: dict[str, object]
) -> None:
    """Remove the protected crates.io environment binding."""
    workflow_job(release, "publish-crates-io").pop("environment", None)


def _drop_crate_package_need(
    release: dict[str, object], _action: dict[str, object]
) -> None:
    """Remove the package artifact producer from publisher dependencies."""
    needs = workflow_job(release, "publish-crates-io").get("needs")
    if isinstance(needs, list) and "package-crate" in needs:
        needs.remove("package-crate")


def _select_unrelated_crate_artifact(
    release: dict[str, object], _action: dict[str, object]
) -> None:
    """Point registry publication at a different artifact name."""
    steps = job_steps(release, "publish-crates-io")
    download = next(
        step for step in steps if "download-artifact@" in str(step.get("uses"))
    )
    require_mapping(download.get("with"), "artifact download.with")["name"] = (
        "release-assets"
    )


def _authenticate_before_classify(
    _release: dict[str, object], action: dict[str, object]
) -> None:
    """Move OIDC exchange before the registry policy has classified state."""
    runs = require_mapping(action.get("runs"), "publisher action runs")
    steps = require_list(runs.get("steps"), "publisher action steps")
    auth_index = next(
        index
        for index, step in enumerate(steps)
        if require_mapping(step, "publisher action step").get("id") == "auth"
    )
    decision_index = next(
        index
        for index, step in enumerate(steps)
        if require_mapping(step, "publisher action step").get("id") == "decision"
    )
    auth = steps.pop(auth_index)
    steps.insert(decision_index, auth)


def _authenticate_without_publish_guard(
    _release: dict[str, object], action: dict[str, object]
) -> None:
    """Remove the publish-only condition from token exchange."""
    auth = next(
        step for step in action_steps(ACTION_PATH, action) if step.get("id") == "auth"
    )
    auth.pop("if", None)


def _rebuild_crate_before_upload(
    _release: dict[str, object], action: dict[str, object]
) -> None:
    """Add a Cargo publish invocation to the composite action."""
    runs = require_mapping(action.get("runs"), "publisher action runs")
    steps = require_list(runs.get("steps"), "publisher action steps")
    steps.append({
        "name": "Rebuild crate",
        "run": "cargo publish",
    })


CRATES_IO_MUTATIONS: dict[
    str, cabc.Callable[[dict[str, object], dict[str, object]], None]
] = {
    "remove-crates-io-oidc-permission": _remove_crates_io_oidc,
    "grant-crates-io-contents-write": _grant_crates_io_contents_write,
    "remove-crates-io-environment": _remove_crates_io_environment,
    "drop-crate-package-prerequisite": _drop_crate_package_need,
    "select-unrelated-crate-artifact": _select_unrelated_crate_artifact,
    "authenticate-before-classify": _authenticate_before_classify,
    "authenticate-without-publish-guard": _authenticate_without_publish_guard,
    "rebuild-crate-before-upload": _rebuild_crate_before_upload,
}
