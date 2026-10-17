"""Pin targeted slow-timeout policy to cold Cargo build fixtures.

The CLI configuration fixture reuses the gate graph and ordinary timeout;
``test_execution_coverage_test`` guards its exclusion from the cold-build tier.
"""

import pytest
from nextest_child_cargo_group_invariants import (
    GROUP_FILTER,
    nextest_config,
)
from workflow_loading import require_list, require_mapping


@pytest.mark.parametrize(
    ("test_name", "fixture_name"),
    [
        pytest.param(
            "stub_env_default_does_not_compile",
            "StubEnv compile-fail fixture",
            id="stub-env-compile-fail",
        ),
        pytest.param(
            "split_build_fixture_compiles_through_the_direct_rustc_harness",
            "direct-rustc split-build fixture",
            id="split-build-direct-rustc",
        ),
        pytest.param(
            "packaged_manifest_retains_build_script_sources",
            "packaged manifest fixture",
            id="packaged-manifest",
        ),
    ],
)
def test_cold_build_fixtures_have_targeted_allowance(
    test_name: str, fixture_name: str
) -> None:
    """Keep each cold-build fixture's timeout attached to its exact test."""
    config = nextest_config()
    profiles = require_mapping(config.get("profile"), "nextest profile table")
    default_profile = require_mapping(
        profiles.get("default"), "nextest default profile"
    )
    overrides = require_list(
        default_profile.get("overrides", []), "nextest default profile overrides"
    )
    matching = []
    for raw_override in overrides:
        override = require_mapping(raw_override, "nextest default override")
        selector = GROUP_FILTER.fullmatch(str(override.get("filter", "")))
        selects_fixture_only = selector is not None and selector.group(1) == test_name
        has_timeout_allowance = "slow-timeout" in override
        if selects_fixture_only and has_timeout_allowance:
            matching.append(override)
    assert len(matching) == 1, f"{fixture_name} must have one targeted timeout override"
    assert matching[0].get("slow-timeout") == {
        "period": "60s",
        "terminate-after": 15,
    }, f"the cold {fixture_name} needs its measured 900 s allowance"
