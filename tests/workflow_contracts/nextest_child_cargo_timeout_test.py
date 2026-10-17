"""Pin the slow-timeout budget of every test that pays a cold Cargo build.

Two shapes live here. The targeted fixtures — the StubEnv compile-fail, the
direct-rustc split build, and the packaged manifest — each carry a
``slow-timeout`` override naming that test alone. The ambient public-API
fixtures share one override instead, because they reach the same
`NetsukeRlib::build()` child `cargo check` against the ambient target
directory and the ``nested-cargo-builds`` group serializes them: the first to
run pays the cold metadata build and the rest reuse it, so the cost belongs
to the family rather than to whichever name the scheduler started first.

This module is the home of the budget map because it owns the question "what
may a cold-build child Cargo test take?". ``test_execution_coverage_test``
owns the single-execution rule and reads none of it.
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


#: The ambient public-API fixtures, in the order the group's filter names
#: them. Written out rather than derived from the filter, because the filter is
#: what this contract holds: deriving would compare the configuration with
#: itself and pass whatever the override said. A name added to or removed from
#: the family has to be added or removed here, and the failure then names the
#: selector that changed rather than printing one opaque expression.
AMBIENT_PUBLIC_API_FIXTURES = (
    "command_env_embedder_fixture_compiles",
    "manifest_environment_embedder_fixture_compiles",
    "policy_from_str_embedder_fixture_compiles_without_clap",
    "cli_configuration_fixture_compiles",
    "command_list_public_api_fixture_compiles",
    "ir_gen_error_public_api_fixture_compiles",
    "legacy_recipe_telemetry_public_api_fixture_compiles",
    "config_cached_discovery_embedder_fixture_compiles",
    "verbose_timing_reporter_embedder_fixture_compiles",
)


def test_cold_build_budgets_are_scoped_to_the_tests_that_pay_them() -> None:
    """Allow measured cold builds without relaxing any other test.

    Every budget in the map below has a measured cost behind it: the profile's
    own 300 s, ten periods for the mutation compile gate, and fifteen (900 s)
    for the ambient family, the StubEnv compile-fail, the split build, and the
    packaged manifest. Asserting the whole map rather than a count means a
    budget that moved, widened silently, or disappeared from an override fails
    here and names itself.
    """
    config = nextest_config()
    profile = config["profile"]["default"]
    assert profile["slow-timeout"] == {"period": "60s", "terminate-after": 5}, (
        "ordinary tests must retain their five-minute budget"
    )
    timed_overrides = [
        override for override in profile["overrides"] if "slow-timeout" in override
    ]
    assert len(timed_overrides) == 5, (
        "preserve current-main cold-build budgets, including the ambient fixture "
        "family's shared cold check"
    )
    configured = {
        override["filter"]: override["slow-timeout"] for override in timed_overrides
    }
    expected = {
        "test(/^packaged_manifest_retains_build_script_sources($|::)/)": {
            "period": "60s",
            "terminate-after": 15,
        },
        (
            "test(/^compile_guard::"
            "every_patched_tree_compiles_under_denied_warnings($|::)/)"
        ): {
            "period": "60s",
            "terminate-after": 10,
        },
    }
    expected.update({
        f"test(/^{name}($|::)/)": {"period": "60s", "terminate-after": periods}
        for name, periods in (
            ("split_build_fixture_compiles_through_the_direct_rustc_harness", 15),
            ("stub_env_default_does_not_compile", 15),
        )
    })
    expected[
        " | ".join(f"test(/^{name}($|::)/)" for name in AMBIENT_PUBLIC_API_FIXTURES)
    ] = {
        "period": "60s",
        "terminate-after": 15,
    }
    assert configured == expected, (
        "cold-build budgets must retain current-main allowances"
    )
