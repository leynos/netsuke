"""Define bounded operation, phase, and outcome labels for release telemetry.

This catalogue owns the fixed low-cardinality vocabulary consumed by
``release_operation_observability``. Add a phase only when a release job
reports that boundary, and list it only for operations that actually run it.
Workflow contracts mirror the phase membership and guard its use.
The publisher context selector permits only validated crate/version strings
and fixed registry state, action, and verification labels in trace records.
"""

import re
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

VALID_OUTCOMES = frozenset({"success", "failure", "cancelled", "skipped", "unknown"})
VALID_STATUS_CLASSES = frozenset({"1xx", "2xx", "3xx", "4xx", "5xx", "unknown"})
REGISTRY_STATES = frozenset({
    "absent",
    "identical",
    "conflict",
    "yanked",
    "indeterminate",
})
REGISTRY_ACTIONS = frozenset({"publish", "skip", "fail"})
_CRATE_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9_-]{0,63}")
_VERSION = re.compile(r"[A-Za-z0-9][A-Za-z0-9.+-]{0,127}")


def publisher_context(
    # ruff: ignore[quoted-annotation] - Python 3.12 evaluates annotations eagerly.
    outputs: "cabc.Mapping[str, object]",
) -> dict[str, str]:
    """Select bounded registry context from publisher action outputs.

    Parameters
    ----------
    outputs
        The action's declared outputs, including phase outcomes.

    Returns
    -------
    dict[str, str]
        Safe crate identity and fixed registry decision fields.

    Examples
    --------
    >>> publisher_context({"registry-state": "absent"})["registry_state"]
    'absent'
    """
    context: dict[str, str] = {}
    for field, output, pattern in (
        ("crate_name", "crate-name", _CRATE_NAME),
        ("version", "crate-version", _VERSION),
    ):
        value = outputs.get(output)
        if isinstance(value, str) and pattern.fullmatch(value):
            context[field] = value

    registry_state = outputs.get("registry-state")
    if isinstance(registry_state, str) and registry_state in REGISTRY_STATES:
        context["registry_state"] = registry_state
    action = outputs.get("action")
    if isinstance(action, str) and action in REGISTRY_ACTIONS:
        context["action"] = action
    verdict = outputs.get("registry-byte-verification-outcome")
    context["verification_verdict"] = (
        verdict if isinstance(verdict, str) and verdict in VALID_OUTCOMES else "unknown"
    )
    return context


PHASES = {
    "draft_release": ("Draft release", "draft_creation"),
    "artifact_download": ("Artefact download", "artifact_download"),
    "archive_hoist": ("Archive hoist", "archive_hoist"),
    "upload_plan_validation": ("Upload plan validation", "upload_plan_validation"),
    "crate_packaging": ("Crate packaging", "crate_packaging"),
    "artifact_upload": ("Artefact upload", "artifact_upload"),
    "crate_candidate_validation": (
        "Crate candidate validation",
        "crate_candidate_validation",
    ),
    "registry_probe": ("Registry probe", "registry_probe"),
    "registry_decision": ("Registry decision", "registry_decision"),
    "trusted_publishing_authentication": (
        "Trusted Publishing authentication",
        "trusted_publishing_authentication",
    ),
    "registry_metadata_derivation": (
        "Registry metadata derivation",
        "registry_metadata_derivation",
    ),
    "registry_upload": ("Registry upload", "registry_upload"),
    "registry_download": ("Registry download", "registry_download"),
    "registry_byte_verification": (
        "Registry byte verification",
        "registry_byte_verification",
    ),
}
OPERATION_PHASES = {
    "release_staging": (
        "draft_release",
        "artifact_download",
        "archive_hoist",
        "upload_plan_validation",
    ),
    "release_publication": (
        "draft_release",
        "artifact_download",
        "archive_hoist",
        "upload_plan_validation",
    ),
    "release_package": ("crate_packaging", "artifact_upload"),
    "publish_crates_io": (
        "crate_candidate_validation",
        "registry_probe",
        "registry_decision",
        "trusted_publishing_authentication",
        "registry_metadata_derivation",
        "registry_upload",
        "registry_download",
        "registry_byte_verification",
    ),
}
OPERATIONS = frozenset(OPERATION_PHASES)
PHASE_ORDER = tuple(PHASES)
PHASE_OUTCOMES = frozenset({"success", "failure", "cancelled"})
