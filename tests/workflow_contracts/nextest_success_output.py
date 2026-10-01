"""Validate the RFC coverage count's immediate-success-output override.

Only the default profile is required here. Existing child-Cargo contracts own
all-filter syntax and declared-test-name validation; this contract compares the
required filter exactly rather than introducing another selector parser.
"""

import typing as typ

from nextest_child_cargo_group_invariants import NEXTEST_CONFIG as NEXTEST_CONFIG
from workflow_loading import require_list, require_mapping

if typ.TYPE_CHECKING:
    import collections.abc as cabc

IMMEDIATE_OUTPUT_TESTS: typ.Final[tuple[str, ...]] = (
    "coverage_map_status_is_reported",
)
INHERITED_PROFILE: typ.Final[str] = "default"
IMMEDIATE_SUCCESS_OUTPUT: typ.Final[str] = "immediate"


def immediate_output_offences(config: cabc.Mapping[str, object]) -> list[str]:
    """Report missing, duplicate, or altered coverage-output overrides.

    An immediate override prints the unwritten count as soon as the test passes;
    the final output mode instead delays that evidence until the run ends.
    Group and timeout policies belong in their own overrides.

    Returns
    -------
    list[str]
        Diagnostics naming each violated override condition, or an empty list
        when every guarded override satisfies the contract.
    """
    profiles = require_mapping(config.get("profile"), "nextest profile table")
    default = require_mapping(
        profiles.get(INHERITED_PROFILE, {}), "nextest default profile"
    )
    overrides = [
        require_mapping(entry, "nextest default override")
        for entry in require_list(
            default.get("overrides", []), "nextest default profile overrides"
        )
    ]
    offences: list[str] = []
    for test in IMMEDIATE_OUTPUT_TESTS:
        selector = f"test(/^{test}($|::)/)"
        matching = [entry for entry in overrides if entry.get("filter") == selector]
        if len(matching) != 1:
            offences.append(
                f"{test}: expected exactly one default-profile override with "
                f"filter {selector!r}; found {len(matching)}"
            )
            continue
        override = matching[0]
        if override.get("success-output") != IMMEDIATE_SUCCESS_OUTPUT:
            offences.append(
                f"{test}: success-output must be 'immediate'; "
                f"found {override.get('success-output')!r}"
            )
        if "test-group" in override:
            offences.append(f"{test}: output override must not set test-group")
        timeouts = sorted(
            key for key in override if key == "timeout" or key.endswith("-timeout")
        )
        if timeouts:
            offences.append(
                f"{test}: output override must not set timeout fields: {timeouts}"
            )
    return offences
