"""Read the Nextest filters that decide which tests carry the child-Cargo policy.

`cargo-nextest` applies a policy to a test by evaluating an override's filter
against real test names: the ``nested-cargo-builds`` group serializes child
Cargo builds, and other overrides widen a single test's timeout. A filter
naming a test that does not exist selects nothing, and so does a filter written
in a form that cannot match the test as it is named at run time. Both parse
cleanly and leave the policy looking enforced while the test runs under the
defaults.

This is the configuration half, reading `.config/nextest.toml` and constraining
the selector grammar. The Rust-source half — which tests are declared, which are
parameterized, and which reach a child Cargo build — lives in
``nextest_rust_test_discovery`` and is re-exported here.

Both halves are test-only: production code must not import either one.

Run via ``make test-workflow-contracts``.
"""

import re
import tomllib
import typing as typ

from nextest_rust_test_discovery import (
    build_capable_test_names as build_capable_test_names,
)
from nextest_rust_test_discovery import (
    declared_test_names as declared_test_names,
)
from nextest_rust_test_discovery import (
    parameterized_test_names as parameterized_test_names,
)
from nextest_rust_test_discovery import (
    rust_test_sources as rust_test_sources,
)
from workflow_loading import (
    REPO_ROOT,
    WorkflowReadError,
    require_list,
    require_mapping,
)

if typ.TYPE_CHECKING:  # pragma: no cover - imported for annotations only
    from pathlib import Path

NEXTEST_CONFIG = REPO_ROOT / ".config" / "nextest.toml"
CHILD_CARGO_GROUP = "nested-cargo-builds"
NESTED_CARGO_BUILD_TESTS = (
    "command_env_embedder_fixture_compiles",
    "policy_from_str_embedder_fixture_compiles_without_clap",
    "cli_configuration_fixture_compiles",
    "command_list_public_api_fixture_compiles",
    "ir_gen_error_public_api_fixture_compiles",
    "legacy_recipe_telemetry_public_api_fixture_compiles",
    "config_cached_discovery_embedder_fixture_compiles",
    "verbose_timing_reporter_embedder_fixture_compiles",
    "production_build_module_slice_has_expected_boundary",
    "stub_env_default_does_not_compile",
    "stub_env_builders_compile_under_the_same_harness",
    "split_build_fixture_compiles_through_the_direct_rustc_harness",
    "text_domains_cannot_be_swapped",
    "document_and_needle_compile_together",
    "packaged_manifest_retains_build_script_sources",
)
# Nextest names a parameterized `#[rstest]` by its instances, `name::case_1_…`.
# `test(=NAME)` compares the whole name, so it matches none of them and the test
# silently leaves the group; `test(/^NAME($|::)/)` matches the plain name and
# every case suffix alike. The `~` substring form is unanchored and over-matches.
GROUP_FILTER = re.compile(r"test\(/\^([a-z0-9_]+)\(\$\|::\)/\)")
LEGACY_EXACT_FILTER = re.compile(r"test\(=([a-z0-9_]+)\)")
# A second deny-list entry beside `LEGACY_EXACT_FILTER` would only ever see the
# spellings it happens to list, so the constraint is the accepted grammar
# instead: every `test(...)` argument is one of the named anchors. `test(=NAME)`
# compares the whole name and misses every case instance, `test(~NAME)` and the
# bare `test(NAME)` that means it are unanchored and over-match, and any other
# argument is a form this file has not reviewed. Scanning each occurrence of
# the `test` keyword lets a filter be reported when it holds an accepted
# selector beside an unanchored one.
TEST_SELECTOR = re.compile(r"\btest\(")
ACCEPTED_TEST_SELECTOR = re.compile(
    r"^/\^[a-z0-9_]+\(\$\|::\)/$"  # `test(/^NAME($|::)/)`, the form used here
    r"|^/\^[a-z0-9_]+\(::\|\$\)/$"  # the same set with the branches transposed
    r"|^=\^[a-z0-9_]+\(::\|\$\)/$"  # `test(=^NAME(::|$)/)`, nextest's own spelling
)


def nextest_config(source: Path | None = None) -> dict[str, object]:
    """Return the parsed Nextest configuration, or raise.

    Parameters
    ----------
    source
        The configuration to read; the repository's `.config/nextest.toml` by
        default. Naming another lets a probe confirm how each read failure is
        reported without provoking one in the shared configuration.

    Returns
    -------
    dict[str, object]
        The parsed document.

    Raises
    ------
    WorkflowReadError
        If the configuration is missing, does not decode as UTF-8, or is not
        TOML. Reading it is fallible in three ways that look nothing alike from
        the caller, and the contract several frames away explains itself better
        when all three arrive as one error naming the path.
    """
    config_path = NEXTEST_CONFIG if source is None else source
    try:
        text = config_path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        message = f"{config_path} could not be read: {error}"
        raise WorkflowReadError(message) from error
    try:
        return tomllib.loads(text)
    except tomllib.TOMLDecodeError as error:
        message = f"{config_path} is not valid TOML: {error}"
        raise WorkflowReadError(message) from error


def all_overrides(config: dict[str, object]) -> list[dict[str, object]]:
    """Return the parsed override tables of every Nextest profile.

    A non-default profile inherits the default table and appends its own
    overrides, so the policies a test can end up carrying are the union across
    profiles rather than the default profile's alone. Reading only `default`
    would leave a filter that silently selects nothing in another profile
    invisible to every contract below.

    Returns
    -------
    list[dict[str, object]]
        Every override table declared under any `[profile.*.overrides]`, in
        profile-name order, narrowed from the parsed document so callers can
        read fields without re-narrowing.
    """
    profiles = require_mapping(config.get("profile"), "nextest profile table")
    overrides: list[dict[str, object]] = []
    for raw_name in sorted(profiles):
        name = str(raw_name)
        profile = require_mapping(profiles[raw_name], f"nextest {name} profile")
        declared = profile.get("overrides")
        if declared is None:
            continue
        overrides.extend(
            require_mapping(override, "nextest override")
            for override in require_list(declared, f"nextest {name} profile overrides")
        )
    return overrides


def group_filter_text(config: dict[str, object]) -> list[str]:
    """Return every filter assigned to the nested-Cargo test group.

    The lookup defaults rather than raising, so an override assigned to the
    group without a `filter` contributes an empty string rather than being
    filtered out: a member that lost its selector is retained as an empty
    entry, and an empty entry selects no names.

    Parameters
    ----------
    config
        The parsed Nextest configuration, as `nextest_config` returns it. Every
        profile is read, not only `default`.

    Returns
    -------
    list[str]
        The filter expression of every override assigning the group, in
        profile-name order, empty for a member that carries none.
    """
    return [
        str(override.get("filter", ""))
        for override in all_overrides(config)
        if override.get("test-group") == CHILD_CARGO_GROUP
    ]


def all_filter_text(config: dict[str, object]) -> list[str]:
    """Return every filter in every profile, grouped or not.

    Serialization is not the only policy a filter carries: a `slow-timeout`
    override selects its test the same way, so the same naming-form mistake
    silently withdraws a widened budget instead of withdrawing a group slot.

    Returns
    -------
    list[str]
        The filter expression of every override that carries one, whether or
        not that override also assigns a test group.
    """
    return [
        str(override["filter"])
        for override in all_overrides(config)
        if "filter" in override
    ]


def filter_test_names(config: dict[str, object]) -> set[str]:
    """Return every test name named by any filter in any profile.

    Both selector spellings are read: `GROUP_FILTER` reads the anchored
    `test(/^NAME($|::)/)` form, and `LEGACY_EXACT_FILTER` reads the rejected
    `test(=NAME)` form. A name written in the rejected form is therefore still
    held to the contract that a filtered name resolves to a declared test,
    rather than dropping out of that check.

    Parameters
    ----------
    config
        The parsed Nextest configuration, as `nextest_config` returns it. Every
        profile is read, not only `default`.

    Returns
    -------
    set[str]
        Every name either pattern reads out of any profile's filters.
    """
    return {
        name
        for filter_ in all_filter_text(config)
        for name in GROUP_FILTER.findall(filter_) + LEGACY_EXACT_FILTER.findall(filter_)
    }


def _test_selector_arguments(filter_: str) -> list[str]:
    """Return the argument of every `test(...)` selector in a filter.

    The parentheses are tracked rather than split on: an accepted argument
    contains `($|::)`, so the first `)` in the filter ends neither the
    argument nor the selector.

    Returns
    -------
    list[str]
        Each selector's argument, in the order the selectors appear, so a
        filter holding several can be judged one selector at a time.
    """
    arguments: list[str] = []
    for match in TEST_SELECTOR.finditer(filter_):
        depth = 1
        index = match.end()
        while index < len(filter_) and depth:
            depth += {"(": 1, ")": -1}.get(filter_[index], 0)
            index += 1
        # An unbalanced argument runs to the end of the filter, and the
        # remainder is reported whole rather than losing its last character.
        arguments.append(filter_[match.end() : index if depth else index - 1])
    return arguments


def _unnamed_test_selectors(filter_: str) -> list[str]:
    """Return the argument of every `test(...)` selector that is not accepted.

    A selector the exact-name pattern already reads a name out of is left to
    that pattern, so each defect keeps one message: the legacy form is reported
    by name, and every other unaccepted form by its whole argument, because
    there is no name in it to report.

    Returns
    -------
    list[str]
        Each unaccepted argument, in the order the selectors appear.
    """
    unaccepted: list[str] = []
    for argument in _test_selector_arguments(filter_):
        if ACCEPTED_TEST_SELECTOR.match(argument):
            continue
        if LEGACY_EXACT_FILTER.fullmatch(f"test({argument})"):
            continue
        unaccepted.append(argument)
    return unaccepted


def unaccepted_test_selectors(config: dict[str, object]) -> dict[str, list[str]]:
    """Return every filter's `test(...)` selectors that are not accepted.

    A deny-list of known-bad forms can only reject the spellings it lists, so
    the contract is the accepted grammar: a filter naming tests must use one of
    the anchored forms above. Anything else — `test(=NAME)`, `test(~NAME)`, the
    bare `test(NAME)` that means `test(~NAME)`, a differently anchored regex,
    or a form not yet reviewed — selects the wrong tests or none at all.

    Returns
    -------
    dict[str, list[str]]
        Filter expression to its unaccepted selector arguments, omitting the
        filters whose every `test(...)` selector is accepted.
    """
    unaccepted = {
        filter_: _unnamed_test_selectors(filter_) for filter_ in all_filter_text(config)
    }
    return {filter_: found for filter_, found in unaccepted.items() if found}


def grouped_test_names(config: dict[str, object]) -> set[str]:
    """Return the test names assigned to the nested-Cargo test group.

    Every profile is read, not only `default`: the group serializes its
    members whichever profile carries the override, so a profile-scoped
    override that left the group would otherwise drop out of the coverage
    contracts without failing one.

    Returns
    -------
    set[str]
        The names the group's filters select.
    """
    return {
        name
        for filter_ in group_filter_text(config)
        for name in GROUP_FILTER.findall(filter_)
    }
