"""Keep nested Cargo builds serialized without reducing suite parallelism.

`cargo-nextest` serializes the tests in its ``nested-cargo-builds`` group so
that child Cargo builds never contend for the build budget. Membership is
decided by Nextest evaluating each override's filter against real test names,
so a filter that names a nonexistent test, or names one in a form that cannot
match how it is named at run time, silently selects nothing and leaves the
test running unserialized while the group still looks healthy.

The discovery half — reading the configuration and classifying the Rust
sources — lives in ``nextest_child_cargo_group_invariants``.

Run via ``make test-workflow-contracts``.
"""

import pytest
from nextest_child_cargo_group_invariants import (
    CHILD_CARGO_GROUP,
    GROUP_FILTER,
    LEGACY_EXACT_FILTER,
    NESTED_CARGO_BUILD_TESTS,
    build_capable_test_names,
    declared_test_names,
    group_filter_text,
    grouped_test_names,
    nextest_config,
    parameterised_test_names,
    rust_test_sources,
)
from workflow_loading import (
    REPO_ROOT,
    load_workflow,
    require_mapping,
    workflow_job,
)


def test_nested_cargo_group_serializes_build_capable_tests() -> None:
    """The named group serializes every current nested Cargo build test."""
    config = nextest_config()
    groups = require_mapping(config.get("test-groups"), "Nextest test groups")
    group = require_mapping(groups.get(CHILD_CARGO_GROUP), "nested Cargo test group")
    assert group.get("max-threads") == 1, (
        "nested Cargo builds must run one at a time on four-vCPU coverage runners"
    )
    grouped = grouped_test_names(config)
    assert set(NESTED_CARGO_BUILD_TESTS) <= grouped, (
        f"nested Cargo build tests missing from {CHILD_CARGO_GROUP}: "
        f"{sorted(set(NESTED_CARGO_BUILD_TESTS) - grouped)!r}"
    )
    assert {
        "cli_configuration_fixture_compiles",
        "packaged_manifest_retains_build_script_sources",
    } <= grouped, "the two isolated nested Cargo tests must be serialized"
    immediate = [
        override
        for override in config["profile"]["default"]["overrides"]
        if override.get("test-group") == CHILD_CARGO_GROUP
    ]
    assert immediate, "nested Cargo build tests must have explicit overrides"
    assert all(
        override.get("success-output") == "immediate" for override in immediate
    ), "nested Cargo build diagnostics must remain immediate"


def test_nested_cargo_group_covers_discovered_build_commands() -> None:
    """Direct and helper-mediated build-capable Cargo commands remain grouped."""
    sources = rust_test_sources()
    discovered = set().union(
        *(build_capable_test_names(source) for source in sources.values())
    )
    assert discovered <= grouped_test_names(nextest_config()), (
        f"build-capable child Cargo tests missing group policy: "
        f"{sorted(discovered - grouped_test_names(nextest_config()))!r}"
    )


def test_group_filters_match_every_declared_test_they_name() -> None:
    """A grouped name must resolve to a declared test.

    A filter naming a test that no Rust source declares selects nothing, yet
    parses cleanly, so the group looks healthy while the test it was meant to
    serialize runs alongside everything else.
    """
    grouped = grouped_test_names(nextest_config())
    declared = declared_test_names()
    unresolved = sorted(grouped - declared.keys())
    assert not unresolved, (
        f"group filters name tests that no Rust source declares: {unresolved!r}; "
        "a filter naming a nonexistent test silently selects nothing"
    )


def test_group_filter_form_matches_parameterised_test_instances() -> None:
    """A parameterised test cannot be selected by the exact-name filter form.

    `#[rstest]` with `#[case]` attributes compiles to one test per case, named
    `name::case_1_…`. Nextest's `test(=NAME)` matches the whole name only, so it
    selects none of those instances and the test escapes serialization; the
    anchored `test(/^NAME($|::)/)` form matches the plain name and every case
    suffix. The legacy form is rejected so that escape cannot return.
    """
    filters = " | ".join(group_filter_text(nextest_config()))
    parameterised = set().union(
        *(parameterised_test_names(source) for source in rust_test_sources().values())
    )
    named_by_legacy = set(LEGACY_EXACT_FILTER.findall(filters)) & parameterised
    assert not named_by_legacy, (
        f"parameterised tests cannot be grouped with the exact-name form, which "
        f"matches none of their cases: {sorted(named_by_legacy)!r}"
    )
    named_by_group = set(GROUP_FILTER.findall(filters)) & parameterised
    assert named_by_group, (
        "the parameterised build-capable test must be named by a case-matching "
        "filter, so its instances are serialized"
    )


def test_no_group_filter_uses_the_exact_name_form() -> None:
    """Every group filter uses the anchored case-matching form."""
    filters = group_filter_text(nextest_config())
    assert filters, "the nested Cargo test group must carry at least one filter"
    legacy = {
        name for filter_ in filters for name in LEGACY_EXACT_FILTER.findall(filter_)
    }
    assert not legacy, (
        f"group filters must use 'test(/^NAME($|::)/)', which matches a "
        f"parameterised test's cases as well as its plain name; found the "
        f"exact-name form for: {sorted(legacy)!r}"
    )


def test_direct_child_cargo_build_addition_requires_group_policy() -> None:
    """A new direct build-capable Cargo test is discovered before it can run."""
    source = """
#[rstest]
fn new_fixture_compiles() {
    Command::new(cargo()).arg("build");
}
"""
    assert build_capable_test_names(source) == {"new_fixture_compiles"}, (
        "the discovery contract must classify direct Cargo build commands"
    )


def test_build_capable_discovery_supports_rust_test_signatures() -> None:
    """Qualified and asynchronous tests cannot evade the child-Cargo policy."""
    source = """
#[test]
pub(crate) fn restricted_fixture_compiles() {
    Command::new(cargo()).arg("build");
}

#[rstest]
async fn asynchronous_fixture_compiles() {
    Command::new(cargo()).arg("check");
}
"""
    assert build_capable_test_names(source) == {
        "asynchronous_fixture_compiles",
        "restricted_fixture_compiles",
    }, "qualified and asynchronous build-capable tests must be discovered"


def test_build_capable_discovery_supports_indented_helpers() -> None:
    """An indented helper cannot hide a child Cargo build from its test caller."""
    source = """
#[test]
fn helper_fixture_compiles() {
    Fixture::build();
}

impl Fixture {
    fn build() {
        Command::new(cargo()).arg("build");
    }
}
"""
    assert build_capable_test_names(source) == {"helper_fixture_compiles"}, (
        "indented build-capable helpers must classify their test callers"
    )


def test_build_capable_discovery_propagates_across_multiple_helper_layers() -> None:
    """A fixture user inherits build capability through every helper layer."""
    source = """
#[rstest]
fn fixture_user(build_fixture: ()) {}

#[fixture]
fn build_fixture() {
    prepare_build();
}

fn prepare_build() {
    launch_build();
}

fn launch_build() {
    Command::new(cargo()).arg("build");
}
"""
    assert build_capable_test_names(source) == {"fixture_user"}, (
        "fixed-point propagation must reach fixture users through helper layers"
    )


def test_parameterised_discovery_sees_case_attributes() -> None:
    """A `#[case]`-parameterised test is recognised as multiply-instantiated."""
    source = """
#[rstest]
#[case::first(1)]
#[case::second(2)]
fn case_fixture_compiles(#[case] value: u32) {
    Command::new(cargo()).arg("build");
}
"""
    assert parameterised_test_names(source) == {"case_fixture_compiles"}, (
        "case attributes must mark a test as parameterised"
    )


@pytest.mark.parametrize(
    ("workflow", "job"),
    [("ci.yml", "build-test"), ("coverage-main.yml", "coverage-upload")],
)
def test_linux_nextest_concurrency_remains_four(workflow: str, job: str) -> None:
    """The serialized subgroup leaves normal Linux Nextest concurrency unchanged."""
    env = require_mapping(
        workflow_job(
            load_workflow(REPO_ROOT / ".github" / "workflows" / workflow), job
        ).get("env"),
        f"{workflow} {job} environment",
    )
    assert env.get("NEXTEST_TEST_THREADS") == "4", (
        f"{workflow} {job} must retain four normal Nextest test threads"
    )
