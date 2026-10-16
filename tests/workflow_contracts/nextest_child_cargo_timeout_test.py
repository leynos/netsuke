"""Pin targeted slow-timeout policy to cold Cargo build fixtures."""

import pytest
from nextest_child_cargo_group_invariants import (
    GROUP_FILTER,
    all_overrides,
    nextest_config,
)


@pytest.mark.parametrize(
    ("test_name", "fixture_name"),
    [
        pytest.param(
            "cli_configuration_fixture_compiles",
            "CLI configuration fixture",
            id="cli-configuration",
        ),
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
    matching = [
        override
        for override in all_overrides(config)
        if test_name in GROUP_FILTER.findall(str(override.get("filter", "")))
        and "slow-timeout" in override
    ]
    assert len(matching) == 1, f"{fixture_name} must have one targeted timeout override"
    assert matching[0].get("slow-timeout") == {
        "period": "60s",
        "terminate-after": 15,
    }, f"the cold {fixture_name} needs its measured 900 s allowance"
