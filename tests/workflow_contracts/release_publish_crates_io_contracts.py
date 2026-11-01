"""Assert that the crates.io job consumes one candidate and gates every effect.

These contracts keep artifact selection, OIDC exchange, upload sequencing, and
download verification visible in the workflow and composite action.
"""

import re
import typing as typ

from action_run_sources import action_steps
from workflow_loading import (
    job_steps,
    require_mapping,
    workflow_job,
)
from workflow_variable_scan import expression_references

if typ.TYPE_CHECKING:
    import collections.abc as cabc

ACTION_PATH = ".github/actions/publish-crates-io/action.yml"
AUTH_ACTION = "rust-lang/crates-io-auth-action@3268eb413133e2960fa4ae0e963ca76380c54cd9"
CHECKOUT_ACTION = "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"
SETUP_UV_ACTION = "astral-sh/setup-uv@1c37ad07a6a961277cf70c0d37d6f313000f5884"
DOWNLOAD_ACTION = "actions/download-artifact@9000827ccba6bdab643e8b6fd33ac0654aef8333"
PUBLISHER_NEEDS = {
    "metadata",
    "package-crate",
    "build-linux",
    "build-windows",
    "build-macos",
    "windows-native-recipe-smoke",
    "release-admission-canaries",
}
ACTION_INPUTS = {
    "identity-path",
    "archive-directory",
    "tag",
    "commit",
    "package-name",
    "version",
}
ACTION_OUTPUTS = {"action", "crate-name", "sha256", "verified-sha256"}
ACTION_STEP_ORDER = (
    "candidate",
    "probe",
    "decision",
    "auth",
    "metadata",
    "upload",
    "download",
    "verify",
)
PUBLISH_ONLY = "steps.decision.outputs.action == 'publish'"
TOKEN_OUTPUT = "$" + "{{ steps.auth.outputs.token }}"
CANDIDATE_PACKAGE_OUTPUT = "$" + "{{ steps.candidate.outputs.package-name }}"
FORBIDDEN_CARGO_COMMAND = re.compile(r"\bcargo\s+(?:package|publish|build)\b")


def _strings(value: object) -> cabc.Iterator[str]:
    """Yield strings nested in parsed action or workflow values."""
    match value:
        case str() as string:
            yield string
        case dict() as mapping:
            for nested in mapping.values():
                yield from _strings(nested)
        case list() as items:
            for nested in items:
                yield from _strings(nested)


def _unique_download_step(
    steps: list[dict[str, object]], violations: list[str]
) -> dict[str, object] | None:
    """Return the sole artifact download step, reporting missing or duplicate use."""
    downloads = [
        step
        for step in steps
        if str(step.get("uses", "")).startswith("actions/download-artifact@")
    ]
    if len(downloads) != 1:
        violations.append(
            "publish-crates-io.artifact: expected one dedicated artifact download"
        )
        return None
    return downloads[0]


def _check_download_values(download: dict[str, object], violations: list[str]) -> None:
    """Require the exact artifact name, path, options, and action pin."""
    values = require_mapping(download.get("with"), "crates.io artifact download.with")
    if download.get("uses") != DOWNLOAD_ACTION:
        violations.append("publish-crates-io.artifact: download action pin changed")
    if set(values) != {"name", "path"}:
        violations.append(
            "publish-crates-io.artifact: use only the exact name and dedicated path"
        )
    if values.get("name") != "release-crate-package":
        violations.append(
            "publish-crates-io.artifact: expected release-crate-package exactly"
        )
    if values.get("path") != "release-crate-package":
        violations.append(
            "publish-crates-io.artifact: download into release-crate-package"
        )
    _check_no_download_merging(values, violations)


def _check_no_download_merging(
    values: dict[str, object], violations: list[str]
) -> None:
    """Reject pattern selection and artifact merging."""
    if "pattern" in values or "merge-multiple" in values:
        violations.append(
            "publish-crates-io.artifact: patterns and merging are forbidden"
        )


def _check_local_action_inputs(
    steps: list[dict[str, object]], violations: list[str]
) -> None:
    """Require exactly the declared local action and candidate paths."""
    local_actions = [
        step
        for step in steps
        if step.get("uses") == "./.github/actions/publish-crates-io"
    ]
    if len(local_actions) != 1:
        violations.append("publish-crates-io.action: local publisher action is missing")
        return
    inputs = require_mapping(local_actions[0].get("with"), "publisher action inputs")
    if set(inputs) != ACTION_INPUTS:
        violations.append(
            "publish-crates-io.action-inputs: expected exactly six inputs"
        )
    expected_paths = {
        "identity-path": "release-crate-package/artifacts/package/identity.json",
        "archive-directory": "release-crate-package/target/package",
    }
    for name, expected in expected_paths.items():
        value = inputs.get(name)
        if value != expected or not str(value).startswith("release-crate-package/"):
            violations.append(
                f"publish-crates-io.candidate: {name} must stay in the download"
                " directory"
            )


def _check_job_artifact(
    release: dict[str, object], violations: list[str]
) -> tuple[list[dict[str, object]], dict[str, object]]:
    """Require the job to download and pass the package-crate candidate."""
    job = workflow_job(release, "publish-crates-io")
    steps = job_steps(release, "publish-crates-io")
    download = _unique_download_step(steps, violations)
    if download is not None:
        _check_download_values(download, violations)
    _check_local_action_inputs(steps, violations)
    return steps, job


def _ordered_action_steps(
    action: dict[str, object], violations: list[str]
) -> tuple[list[dict[str, object]], dict[str, int]]:
    """Return action steps and the index of each required step identifier."""
    steps = action_steps(ACTION_PATH, action)
    identifiers = [str(step.get("id", "")) for step in steps]
    positions = {
        name: identifiers.index(name) if name in identifiers else -1
        for name in ACTION_STEP_ORDER
    }
    ordered = [positions[name] for name in ACTION_STEP_ORDER]
    if -1 in ordered or ordered != sorted(ordered):
        violations.append(
            "publish-crates-io.action-order: expected candidate, probe, classify, "
            "auth, metadata, upload, download, verify"
        )
    return steps, positions


def _check_action_effect_gates(
    steps: list[dict[str, object]], positions: dict[str, int], violations: list[str]
) -> None:
    """Gate token exchange and mutation while verifying publish and skip."""
    if -1 in positions.values():
        return
    auth = steps[positions["auth"]]
    if auth.get("uses") != AUTH_ACTION:
        violations.append(
            "publish-crates-io.auth: trusted-publisher action pin changed"
        )
    _check_publish_only_steps(steps, positions, violations)
    _check_unconditional_verification(steps, positions, violations)
    _check_candidate_and_classification(steps, positions, violations)
    _check_upload_token_and_download_identity(steps, positions, violations)


def _check_publish_only_steps(
    steps: list[dict[str, object]], positions: dict[str, int], violations: list[str]
) -> None:
    """Require the auth exchange and upload preparation to follow publish only."""
    guarded_steps = ("auth", "metadata", "upload")
    violations.extend(
        f"publish-crates-io.{name}-guard: require the publish decision"
        for name in guarded_steps
        if steps[positions[name]].get("if") != PUBLISH_ONLY
    )


def _check_unconditional_verification(
    steps: list[dict[str, object]], positions: dict[str, int], violations: list[str]
) -> None:
    """Require both skip and publish decisions to download and verify bytes."""
    verification_steps = ("download", "verify")
    violations.extend(
        f"publish-crates-io.{name}-guard: must run for publish and skip"
        for name in verification_steps
        if "if" in steps[positions[name]]
    )


def _check_candidate_and_classification(
    steps: list[dict[str, object]], positions: dict[str, int], violations: list[str]
) -> None:
    """Require candidate validation and offline policy classification commands."""
    candidate_run = str(steps[positions["candidate"]].get("run", ""))
    candidate_fragments = (
        "crates_io_candidate_cli.py check",
        "jq -r '.crate'",
        "jq -r '.identity.package_name'",
    )
    if any(fragment not in candidate_run for fragment in candidate_fragments):
        violations.append(
            "publish-crates-io.candidate: validate and read the identity record fields"
        )
    decision_run = str(steps[positions["decision"]].get("run", ""))
    if "publish_crates_io_cli.py" not in decision_run or "classify" not in decision_run:
        violations.append("publish-crates-io.classify: offline policy CLI is required")


def _check_upload_token_and_download_identity(
    steps: list[dict[str, object]], positions: dict[str, int], violations: list[str]
) -> None:
    """Pass the short-lived token to upload and verify the candidate package."""
    upload_env = require_mapping(steps[positions["upload"]].get("env"), "upload.env")
    if upload_env.get("CRATES_IO_TOKEN") != TOKEN_OUTPUT:
        violations.append("publish-crates-io.token: use the short-lived auth output")
    verify = steps[positions["verify"]]
    verify_env = require_mapping(verify.get("env"), "verify.env")
    if (
        '--crate "$CRATE_NAME"' not in str(verify.get("run", ""))
        or verify_env.get("CRATE_NAME") != CANDIDATE_PACKAGE_OUTPUT
    ):
        violations.append(
            "publish-crates-io.verify: verify bytes for identity.package_name"
        )


def _check_forbidden_inputs(
    sources: tuple[tuple[str, object], ...],
    violations: list[str],
) -> None:
    """Reject secret contexts, long-lived tokens and Cargo rebuild commands."""
    for label, value in sources:
        for source in _strings(value):
            if "CARGO_REGISTRY_TOKEN" in source:
                violations.append(f"{label}.token: CARGO_REGISTRY_TOKEN is forbidden")
            if any(
                namespace == "secrets"
                for namespace, _name in expression_references(source, bare=True)
            ):
                violations.append(f"{label}.secrets: secret references are forbidden")
            if FORBIDDEN_CARGO_COMMAND.search(source):
                violations.append(
                    f"{label}.rebuild: Cargo package, publish, or build is forbidden"
                )


def _check_job_permissions(
    release: dict[str, object],
    job: dict[str, object],
    violations: list[str],
) -> None:
    """Keep OIDC authority exclusive to the job declared in this workflow."""
    permissions = require_mapping(job.get("permissions"), "publisher permissions")
    if permissions != {"contents": "read", "id-token": "write"}:
        violations.append(
            "publish-crates-io.permissions: require contents read and id-token write"
        )
    if job.get("environment") != "crates-io":
        violations.append("publish-crates-io.environment: require crates-io")
    jobs = require_mapping(release.get("jobs"), "release jobs")
    oidc_writers = [
        name
        for name, raw_job in jobs.items()
        if _has_oidc_write_permission(raw_job, name)
    ]
    if oidc_writers != ["publish-crates-io"]:
        violations.append(
            "publish-crates-io.permissions: keep id-token write exclusive"
        )
    job_uses = job.get("uses")
    if isinstance(job_uses, str) and job_uses.startswith("./.github/workflows/"):
        violations.append("publish-crates-io.job: declare OIDC exchange in release.yml")


def _has_oidc_write_permission(raw_job: object, name: str) -> bool:
    """Return whether one release job receives OIDC write authority."""
    job = require_mapping(raw_job, f"release job {name}")
    permissions = require_mapping(
        job.get("permissions", {}), f"release job {name} permissions"
    )
    return permissions.get("id-token") == "write"


def _check_action_manifest_shape(
    action: dict[str, object], violations: list[str]
) -> None:
    """Require the composite action's public input and output sets."""
    inputs = require_mapping(action.get("inputs"), "publisher action.inputs")
    outputs = require_mapping(action.get("outputs"), "publisher action.outputs")
    if set(inputs) != ACTION_INPUTS:
        violations.append(
            "publish-crates-io.action-inputs: expected exactly six inputs"
        )
    if set(outputs) != ACTION_OUTPUTS:
        violations.append(
            "publish-crates-io.action-outputs: expected exactly four outputs"
        )


def _check_checkout_and_interpreter(
    steps: list[dict[str, object]], violations: list[str]
) -> None:
    """Require pinned checkout, disabled persistence, and the package UV pin."""
    checkouts = [
        step
        for step in steps
        if str(step.get("uses", "")).startswith("actions/checkout@")
    ]
    if len(checkouts) != 1 or checkouts[0].get("uses") != CHECKOUT_ACTION:
        violations.append("publish-crates-io.checkout: require the pinned checkout")
    elif (
        require_mapping(checkouts[0].get("with"), "publisher checkout.with").get(
            "persist-credentials"
        )
        is not False
    ):
        violations.append(
            "publish-crates-io.checkout: persist-credentials must be false"
        )
    if not any(step.get("uses") == SETUP_UV_ACTION for step in steps):
        violations.append(
            "publish-crates-io.uv: require the package-crate setup-uv pin"
        )


def check_crates_io_action_contract(
    release: dict[str, object],
    action: dict[str, object],
    violations: list[str],
) -> None:
    """Check exact artifact wiring and composite publisher effect gates."""
    steps, job = _check_job_artifact(release, violations)
    _check_job_permissions(release, job, violations)
    _check_action_manifest_shape(action, violations)
    action_steps_value, positions = _ordered_action_steps(action, violations)
    _check_action_effect_gates(action_steps_value, positions, violations)
    _check_forbidden_inputs(
        (
            ("publish-crates-io", [job, steps]),
            ("publish-crates-io.action", [action, action_steps_value]),
        ),
        violations,
    )
    _check_checkout_and_interpreter(steps, violations)
