"""Keep nested Cargo builds serialized without reducing suite parallelism.

`cargo-nextest` applies a policy to a test by evaluating an override's filter
against real test names: the ``nested-cargo-builds`` group serializes child
Cargo builds, and other overrides widen a single test's timeout. A filter that
names a nonexistent test, or names one in a form that cannot match how it is
named at run time, silently selects nothing and leaves the test running under
the defaults while the policy still looks enforced.

The discovery half is split in two: ``nextest_child_cargo_group_invariants``
reads the configuration and constrains the selector grammar, and
``nextest_rust_test_discovery`` classifies the Rust sources. Both are test-only.

Run via ``make test-workflow-contracts``.
"""

import typing as typ

import pytest
from nextest_child_cargo_group_invariants import (
    CHILD_CARGO_GROUP,
    GROUP_FILTER,
    LEGACY_EXACT_FILTER,
    NESTED_CARGO_BUILD_TESTS,
    all_filter_text,
    all_overrides,
    build_capable_test_names,
    declared_test_names,
    filter_test_names,
    grouped_test_names,
    nextest_config,
    parameterized_test_names,
    rust_test_sources,
    unaccepted_test_selectors,
)
from workflow_loading import (
    REPO_ROOT,
    WorkflowReadError,
    load_workflow,
    require_mapping,
    workflow_job,
)

if typ.TYPE_CHECKING:  # pragma: no cover - typing-only import
    from pathlib import Path


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
        for override in all_overrides(config)
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


def test_filters_match_every_declared_test_they_name() -> None:
    """A filtered name must resolve to a declared test.

    A filter naming a test that no Rust source declares selects nothing, yet
    parses cleanly, so the policy it carries looks enforced while the test it
    was meant to cover runs under the defaults. Every filter is checked, not
    only the group's, because a `slow-timeout` override selects its test the
    same way and fails the same way.
    """
    named = filter_test_names(nextest_config())
    declared = declared_test_names()
    unresolved = sorted(named - declared.keys())
    assert not unresolved, (
        f"filters name tests that no Rust source declares: {unresolved!r}; "
        "a filter naming a nonexistent test silently selects nothing"
    )


def test_filters_match_parameterized_test_instances() -> None:
    """A parameterized test cannot be selected by the exact-name filter form.

    `#[rstest]` with `#[case]` attributes compiles to one test per case, named
    `name::case_1_…`. Nextest's `test(=NAME)` matches the whole name only, so it
    selects none of those instances and the test leaves whatever policy named
    it; the anchored `test(/^NAME($|::)/)` form matches the plain name and every
    case suffix. The legacy form is rejected so that escape cannot return.
    """
    filters = " | ".join(all_filter_text(nextest_config()))
    parameterized = set().union(
        *(parameterized_test_names(source) for source in rust_test_sources().values())
    )
    named_by_legacy = set(LEGACY_EXACT_FILTER.findall(filters)) & parameterized
    assert not named_by_legacy, (
        f"parameterized tests cannot be selected with the exact-name form, which "
        f"matches none of their cases: {sorted(named_by_legacy)!r}"
    )
    named_by_group = set(GROUP_FILTER.findall(filters)) & parameterized
    assert named_by_group, (
        "the parameterized build-capable test must be named by a case-matching "
        "filter, so its instances are serialized"
    )


def test_no_filter_uses_the_exact_name_form() -> None:
    """Every filter uses the anchored case-matching form.

    Scoped to all filters rather than the group's alone. A filter that names a
    test which is not parameterized today matches under either form, so the
    distinction is invisible until someone adds a `#[case]` attribute; applying
    one form throughout means that later edit cannot silently unhook the test
    from the policy, whichever override carries it.
    """
    config = nextest_config()
    filters = all_filter_text(config)
    assert filters, "the configuration must carry at least one filter"
    legacy = {
        name for filter_ in filters for name in LEGACY_EXACT_FILTER.findall(filter_)
    }
    assert not legacy, (
        f"filters must use 'test(/^NAME($|::)/)', which matches a "
        f"parameterized test's cases as well as its plain name; found the "
        f"exact-name form for: {sorted(legacy)!r}"
    )
    unaccepted = unaccepted_test_selectors(config)
    assert not unaccepted, (
        f"filters must use an accepted anchored 'test(...)' form, because a "
        f"form outside that grammar either names the wrong tests or none at "
        f"all; found: {unaccepted!r}"
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


def test_parameterized_discovery_sees_case_attributes() -> None:
    """A `#[case]`-parameterized test is recognized as multiply-instantiated."""
    source = """
#[rstest]
#[case::first(1)]
#[case::second(2)]
fn case_fixture_compiles(#[case] value: u32) {
    Command::new(cargo()).arg("build");
}
"""
    assert parameterized_test_names(source) == {"case_fixture_compiles"}, (
        "case attributes must mark a test as parameterized"
    )


def test_a_directory_without_rust_sources_is_refused(tmp_path: Path) -> None:
    """A tree that yields no Rust source is refused rather than read as empty.

    `Path.rglob` yields nothing for a directory that is absent or is not a
    directory, so a reading that trusted the glob would return an empty corpus
    and let every assertion above it pass having read no test at all.
    """
    absent = tmp_path / "absent"
    a_file = tmp_path / "a-file"
    a_file.write_text("not a directory", encoding="utf-8")
    for path in (absent, a_file):
        with pytest.raises(WorkflowReadError, match="not a directory"):
            rust_test_sources(path)


def test_the_anchored_form_is_the_one_the_contracts_admit(tmp_path: Path) -> None:
    """The accepted grammar admits the anchored form and rejects the others.

    Every filter in the repository is written to the anchored grammar, so the
    grammar checks above would pass unchanged if it admitted everything. These
    cases pin the boundary itself: the form this branch repaired to is admitted,
    and the two forms it repaired away from — the whole-name `=` form that
    misses every case instance, and the unanchored `~` form that over-matches —
    are both rejected.
    """
    fixture = tmp_path / "case_fixture.rs"
    fixture.write_text(
        "#[rstest]\n"
        "#[case::first(1)]\n"
        "#[case::second(2)]\n"
        "fn scratch_case_fixture(#[case] value: u32) {\n"
        '    Command::new(cargo()).arg("build");\n'
        "}\n",
        encoding="utf-8",
    )
    sources = rust_test_sources(tmp_path)
    assert set(sources) == {fixture}, "the scratch tree must be the tree read"
    assert parameterized_test_names(sources[fixture]) == {"scratch_case_fixture"}, (
        "the fixture must be recognized as multiply-instantiated, or the case "
        "below would pass because no test was seen to be parameterized at all"
    )

    def config_for(filter_: str) -> dict[str, object]:
        """Return a one-override configuration carrying ``filter_``."""
        return {"profile": {"default": {"overrides": [{"filter": filter_}]}}}

    anchored = "test(/^scratch_case_fixture($|::)/)"
    assert filter_test_names(config_for(anchored)) == {"scratch_case_fixture"}, (
        "the anchored form must yield the base name its cases are named from"
    )
    assert not unaccepted_test_selectors(config_for(anchored)), (
        "the anchored form must be admitted"
    )
    # The whole-name form is rejected too, by the pattern that reads a name out
    # of it: `test_no_filter_uses_the_exact_name_form` consumes that pattern, so
    # that form is reported there by name and is deliberately left out of this
    # one's result rather than being double-reported.
    legacy = config_for("test(=scratch_case_fixture)")
    assert LEGACY_EXACT_FILTER.findall(all_filter_text(legacy)[0]) == [
        "scratch_case_fixture"
    ], "the whole-name form must be rejected, as it selects no case instance"
    assert not unaccepted_test_selectors(legacy), (
        "the whole-name form is the legacy pattern's to report, not this one's"
    )
    assert unaccepted_test_selectors(config_for("test(~scratch_case_fixture)")) == {
        "test(~scratch_case_fixture)": ["~scratch_case_fixture"]
    }, "the unanchored form must be rejected, as it over-matches"
    # A filter may hold an accepted selector beside an unanchored one; each is
    # judged alone, so admitting the first must not excuse the second.
    assert unaccepted_test_selectors(
        config_for(f"{anchored} | test(~scratch_case_fixture)")
    ) == {f"{anchored} | test(~scratch_case_fixture)": ["~scratch_case_fixture"]}, (
        "an accepted selector must not mask an unanchored one beside it"
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
