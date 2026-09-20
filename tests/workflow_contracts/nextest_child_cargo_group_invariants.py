"""Classify the tests that spawn build-capable child Cargo commands.

`cargo-nextest` applies a policy to a test by evaluating an override's filter
against real test names: the ``nested-cargo-builds`` group serializes child
Cargo builds, and other overrides widen a single test's timeout. A filter
naming a test that does not exist selects nothing, and so does a filter written
in a form that cannot match the test as it is named at run time. Both parse
cleanly and leave the policy looking enforced while the test runs under the
defaults.

This module holds the discovery half — reading the Nextest configuration and
classifying the Rust integration tests — so the contract tests in
``nextest_child_cargo_group_test`` can assert on it.

Run via ``make test-workflow-contracts``.
"""

import re
import tomllib
import typing as typ

from rust_source_scan import mask_non_code
from workflow_loading import REPO_ROOT, require_list, require_mapping

if typ.TYPE_CHECKING:
    from pathlib import Path

NEXTEST_CONFIG = REPO_ROOT / ".config" / "nextest.toml"
BUILD_CAPABLE_SUBCOMMANDS = ("build", "check", "package", "publish")
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
CASE_ATTRIBUTE = re.compile(r"#\[case(?:::[a-z0-9_]+)?[\(\[]")
RUST_FUNCTION = re.compile(
    r"(?ms)^(?P<indent>[ \t]*)"
    r"(?P<attributes>(?:(?P=indent)#\[[^\n]*\]\s*|"
    r"(?P=indent)#\[[\s\S]*?^(?P=indent)[^\n]*\]\s*)*)"
    r"(?P<signature>(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?"
    r"fn\s+(?P<name>[a-z0-9_]+)\b[^\{]*)\{"
)
CARGO_COMMAND = re.compile(r"Command::new\([^)]*cargo\w*[^)]*\)", re.IGNORECASE)
CARGO_OPERATION = re.compile(
    rf'\.(?:arg|args)\(\s*(?:\[\s*)?"(?:{"|".join(BUILD_CAPABLE_SUBCOMMANDS)})"'
)
RETAINED_RUST_LITERALS = {
    *(f'"{operation}"' for operation in BUILD_CAPABLE_SUBCOMMANDS),
    '"cargo"',
}
type RustFunction = tuple[str, str, str, str]


def nextest_config() -> dict[str, object]:
    """Return the parsed Nextest configuration."""
    return tomllib.loads(NEXTEST_CONFIG.read_text(encoding="utf-8"))


def default_overrides(config: dict[str, object]) -> list[dict[str, object]]:
    """Return the default profile's parsed override tables.

    Returns
    -------
    list[dict[str, object]]
        Each override table in `[profile.default.overrides]`, narrowed from the
        parsed document so callers can read fields without re-narrowing.
    """
    profile = require_mapping(config.get("profile"), "nextest profile table")
    default = require_mapping(profile.get("default"), "nextest default profile")
    overrides = require_list(
        default.get("overrides"), "default Nextest profile must define overrides"
    )
    return [require_mapping(override, "nextest override") for override in overrides]


def group_filter_text(config: dict[str, object]) -> list[str]:
    """Return every filter assigned to the nested-Cargo test group."""
    return [
        str(override.get("filter", ""))
        for override in default_overrides(config)
        if override.get("test-group") == CHILD_CARGO_GROUP
    ]


def all_filter_text(config: dict[str, object]) -> list[str]:
    """Return every filter in the default profile, grouped or not.

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
        for override in default_overrides(config)
        if "filter" in override
    ]


def filter_test_names(config: dict[str, object]) -> set[str]:
    """Return every test name named by any default-profile filter."""
    return {
        name
        for filter_ in all_filter_text(config)
        for name in GROUP_FILTER.findall(filter_) + LEGACY_EXACT_FILTER.findall(filter_)
    }


def grouped_test_names(config: dict[str, object]) -> set[str]:
    """Return the test names assigned to the nested-Cargo test group."""
    return {
        name
        for filter_ in group_filter_text(config)
        for name in GROUP_FILTER.findall(filter_)
    }


def rust_test_sources() -> dict[Path, str]:
    """Return every Rust integration-test source keyed by repository path."""
    return {
        path: path.read_text(encoding="utf-8")
        for path in REPO_ROOT.joinpath("tests").rglob("*.rs")
    }


def _rust_functions(source: str) -> list[RustFunction]:
    """Return attributes, signatures, names, and source slices for Rust functions."""
    matches = list(RUST_FUNCTION.finditer(source))
    return [
        (
            match.group("attributes"),
            match.group("signature"),
            match.group("name"),
            source[
                match.end() : matches[index + 1].start()
                if index + 1 < len(matches)
                else len(source)
            ],
        )
        for index, match in enumerate(matches)
    ]


def _operation_constants(source: str) -> set[str]:
    """Return constants whose argument arrays name build-capable Cargo operations."""
    constants: set[str] = set()
    pattern = re.compile(
        r"(?ms)^const\s+(?P<name>[A-Z0-9_]+)\s*:[^=]+="
        r"(?P<value>.*?);"
    )
    for match in pattern.finditer(source):
        if any(
            f'"{operation}"' in match.group("value")
            for operation in BUILD_CAPABLE_SUBCOMMANDS
        ):
            constants.add(match.group("name"))
    return constants


def _cargo_wrappers(functions: list[RustFunction]) -> set[str]:
    """Return helper names that create Cargo `Command` values."""
    return {
        name
        for _, signature, name, body in functions
        if "-> Command" in signature and CARGO_COMMAND.search(body)
    }


def _has_build_capable_cargo_command(
    body: str, wrappers: set[str], operation_constants: set[str]
) -> bool:
    """Return whether `body` launches a build-capable Cargo command directly."""
    command_created = bool(CARGO_COMMAND.search(body)) or any(
        re.search(rf"\b{wrapper}\s*\(", body) for wrapper in wrappers
    )
    operation_supplied = bool(CARGO_OPERATION.search(body)) or any(
        re.search(rf"\b{constant}\b", body) for constant in operation_constants
    )
    return command_created and operation_supplied


def _calls_build_helper(body: str) -> bool:
    """Return whether `body` invokes an associated build-capable helper."""
    return bool(re.search(r"\b[A-Za-z0-9_]+::build(?:_with)?\s*\(", body))


def _is_rust_test(attributes: str) -> bool:
    """Return whether Rust attributes mark a test or parameterized test."""
    return "#[test]" in attributes or "#[rstest]" in attributes


def _initial_build_capable_names(
    functions: list[RustFunction], wrappers: set[str], operation_constants: set[str]
) -> set[str]:
    """Return functions that directly launch Cargo or call an associated helper."""
    return {
        name
        for _, _, name, body in functions
        if _has_build_capable_cargo_command(body, wrappers, operation_constants)
        or _calls_build_helper(body)
    }


def _callers_of_build_capable_helpers(
    functions: list[RustFunction], helper_names: set[str]
) -> set[str]:
    """Return functions that call a build-capable helper by its bare name."""
    return {
        name
        for _, _, name, body in functions
        if any(
            helper_name != "build" and re.search(rf"\b{helper_name}\s*\(", body)
            for helper_name in helper_names
        )
    }


def _fixture_users_of_build_capable_helpers(
    functions: list[RustFunction], helper_names: set[str]
) -> set[str]:
    """Return Rust tests that consume a build-capable fixture helper."""
    fixtures = {
        name
        for attributes, _, name, _ in functions
        if "#[fixture]" in attributes and name in helper_names
    }
    return {
        name
        for attributes, signature, name, body in functions
        if _is_rust_test(attributes)
        and any(fixture in signature + body for fixture in fixtures)
    }


def _expand_build_capable_names(
    functions: list[RustFunction], build_capable: set[str]
) -> set[str]:
    """Propagate build capability through callers and fixture users to a fixed point."""
    while True:
        helper_names = {name for _, _, name, _ in functions if name in build_capable}
        expanded = (
            build_capable
            | _callers_of_build_capable_helpers(functions, helper_names)
            | _fixture_users_of_build_capable_helpers(functions, helper_names)
        )
        if expanded == build_capable:
            return build_capable
        build_capable = expanded


def build_capable_test_names(source: str) -> set[str]:
    """Return tests reaching a direct or helper-mediated Cargo build command."""
    executable_source = mask_non_code(source, RETAINED_RUST_LITERALS)
    functions = _rust_functions(executable_source)
    build_capable = _expand_build_capable_names(
        functions,
        _initial_build_capable_names(
            functions,
            _cargo_wrappers(functions),
            _operation_constants(executable_source),
        ),
    )
    return {
        name
        for attributes, _, name, _ in functions
        if name in build_capable and _is_rust_test(attributes)
    }


def parameterized_test_names(source: str) -> set[str]:
    """Return the parameterized tests declared in a Rust source.

    A test with `#[case]` attributes is named `name::case_1_…` at run time, so a
    filter must match the case suffix; the exact `test(=NAME)` form would select
    nothing.

    Returns
    -------
    set[str]
        The names of tests whose `#[case]` attributes multiply them into
        instances.
    """
    executable_source = mask_non_code(source, RETAINED_RUST_LITERALS)
    return {
        name
        for attributes, _, name, _ in _rust_functions(executable_source)
        if _is_rust_test(attributes) and CASE_ATTRIBUTE.search(attributes)
    }


def declared_test_names() -> dict[str, set[str]]:
    """Return every declared test name mapped to the sources declaring it."""
    declared: dict[str, set[str]] = {}
    for path, source in rust_test_sources().items():
        executable_source = mask_non_code(source, RETAINED_RUST_LITERALS)
        for attributes, _, name, _ in _rust_functions(executable_source):
            if _is_rust_test(attributes):
                declared.setdefault(name, set()).add(str(path))
    return declared
