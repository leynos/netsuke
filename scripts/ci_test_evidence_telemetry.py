"""Read optional attempt-bound numeric inventory artefacts for offline analysis.

The collector owns this schema. It accepts at most 10 MiB per file, copies
only fixed non-negative numeric fields, and rejects another attempt's data.
No raw errors, environment dumps, or arbitrary labels enter the evidence.
"""

import json
import math
import typing as typ

from ci_test_evidence_types import INVENTORY_FIELDS, validate_identities

if typ.TYPE_CHECKING:
    from pathlib import Path

    from ci_test_evidence_types import LogEvidence

MAX_BYTES = 10 * 1024 * 1024
INVENTORY_IDENTITY_FIELDS = ("run_id", "run_attempt", "job_id")
NUMERIC_FIELDS = {
    section: frozenset(fields) for section, fields in INVENTORY_FIELDS.items()
}


def load_inventory(path: Path, identity: tuple[int, int, int]) -> dict:
    """Read a bounded JSON inventory matching run, attempt, and job identity.

    Parameters
    ----------
    path
        Local saved metadata artefact.
    identity
        For example, ``(123, 2, 456)`` rejects an attempt-1 inventory.

    Returns
    -------
    dict
        Validated inventory envelope; consumers copy only allow-listed fields.

    Raises
    ------
    ValueError
        If inventory is oversized, malformed, or belongs to another attempt.
    TypeError
        If the decoded envelope is not an object.
    """
    with path.open("rb") as stream:
        data = stream.read(MAX_BYTES + 1)
    if len(data) > MAX_BYTES:
        message = "inventory exceeds metadata budget"
        raise ValueError(message)
    inventory = json.loads(data)
    if not isinstance(inventory, dict):
        message = "inventory must be a JSON object"
        raise TypeError(message)
    return validate_inventory(inventory, identity)


def validate_inventory(
    inventory: object, identity: tuple[int, int, int]
) -> dict[str, object]:
    """Validate an inventory object against its run, attempt, and job.

    Parameters
    ----------
    inventory
        Decoded inventory envelope to validate.
    identity
        Expected ``(run_id, run_attempt, job_id)`` triple, such as
        ``(123, 2, 456)``.

    Reject non-object data, invalid identities, unsupported schemas, and an
    inventory belonging to another run attempt or job.

    Returns
    -------
    dict
        The validated inventory envelope for allow-listed field extraction.

    Raises
    ------
    TypeError
        If ``inventory`` is not a JSON object, or ``identity`` does not
        contain three integers.
    ValueError
        If an identity is not positive, the schema is unsupported, or the
        inventory belongs to another run attempt or job.

    """
    if not _is_inventory_object(inventory):
        message = "inventory must be a JSON object"
        raise TypeError(message)
    _require_valid_identity(identity)
    if not _has_supported_schema(inventory):
        message = "invalid inventory schema"
        raise ValueError(message)
    if not _matches_attempt(inventory, identity):
        message = "inventory attempt mismatch"
        raise ValueError(message)
    return inventory


def _is_inventory_object(value: object) -> typ.TypeGuard[dict[str, object]]:
    """Narrow decoded inventory to an object with JSON string keys."""
    return isinstance(value, dict) and all(isinstance(key, str) for key in value)


def _require_valid_identity(identity: tuple[int, int, int]) -> None:
    """Require positive integer run, attempt, and job identifiers."""
    if len(identity) != len(INVENTORY_IDENTITY_FIELDS):
        message = "inventory identity must contain three integers"
        raise TypeError(message)
    validate_identities(list(identity))


def _has_supported_schema(inventory: dict[str, object]) -> bool:
    """Check that the inventory uses the collector's current schema."""
    return type(inventory.get("schema")) is int and inventory["schema"] == 1


def _matches_attempt(
    inventory: dict[str, object], identity: tuple[int, int, int]
) -> bool:
    """Check that metadata belongs to the current run attempt and job."""
    observed = tuple(inventory.get(key) for key in INVENTORY_IDENTITY_FIELDS)
    return all(type(value) is int for value in observed) and observed == identity


def _is_measurement(value: object) -> typ.TypeGuard[int | float]:
    """Validate scalars; ``_is_measurement(True)`` returns ``False``."""
    match value:
        case bool():
            return False
        case int():
            return 0 <= value <= 2**63 - 1
        case float():
            return math.isfinite(value) and value >= 0
        case _:
            return False


def _measurements(inventory: dict) -> dict[str, dict[str, int | float]]:
    """Select valid values; ``_measurements({})`` gives empty sections."""
    result = {}
    for section, fields in NUMERIC_FIELDS.items():
        observed = inventory.get(section, {})
        if not isinstance(observed, dict):
            continue
        result[section] = {
            field: value
            for field in fields
            if _is_measurement(value := observed.get(field))
            and (field.endswith("_seconds") or isinstance(value, int))
        }
    return result


def apply_inventory(evidence: LogEvidence, inventory: dict) -> None:
    """Update evidence with valid, allow-listed inventory measurements.

    The evidence mapping is changed in place. Only numeric fields named in
    :data:`NUMERIC_FIELDS` are copied; malformed, negative, and non-finite
    measurements remain absent.

    Parameters
    ----------
    evidence
        Log evidence mapping that receives the validated measurements.
    inventory
        Attempt-bound inventory object, normally returned by
        :func:`load_inventory`.

    Examples
    --------
    >>> evidence = {"profiles": {}, "resources": {}, "nested_cargo": {}}
    >>> apply_inventory(evidence, {"profiles": {"raw_count": 2}})
    >>> evidence["profiles"]["raw_count"]
    2
    """
    measurements = _measurements(inventory)
    evidence["profiles"].update(measurements.get("profiles", {}))
    evidence["resources"].update(measurements.get("resources", {}))
    evidence["nested_cargo"].update(measurements.get("nested_cargo", {}))
