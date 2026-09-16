"""Keep nested Cargo builds serialized without reducing suite parallelism.

Run via ``make test-workflow-contracts``.
"""

import re
import tomllib
import typing as typ

import pytest
from workflow_loading import REPO_ROOT, load_workflow, require_mapping, workflow_job

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
    "harness_compiles_under_a_split_build_dir",
    "text_domains_cannot_be_swapped",
    "document_and_needle_compile_together",
    "packaged_manifest_retains_build_script_sources",
)
RUST_FUNCTION = re.compile(
    r"(?ms)^(?P<attributes>(?:#\[[\s\S]*?\]\s*)*)"
    r"(?P<signature>(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?"
    r"fn\s+(?P<name>[a-z0-9_]+)\b[^\{]*)\{"
)
CARGO_COMMAND = re.compile(r"Command::new\([^)]*cargo\w*[^)]*\)", re.IGNORECASE)
CARGO_OPERATION = re.compile(
    rf'\.(?:arg|args)\(\s*(?:\[\s*)?"(?:{"|".join(BUILD_CAPABLE_SUBCOMMANDS)})"'
)


def _nextest_config() -> dict[str, object]:
    """Return the parsed Nextest configuration."""
    return tomllib.loads(NEXTEST_CONFIG.read_text(encoding="utf-8"))


def _default_overrides(config: dict[str, object]) -> list[dict[str, object]]:
    """Return the default profile's parsed override tables."""
    profile = require_mapping(config.get("profile"), "nextest profile table")
    default = require_mapping(profile.get("default"), "nextest default profile")
    overrides = default.get("overrides")
    assert isinstance(overrides, list), "default Nextest profile must define overrides"
    return [require_mapping(override, "nextest override") for override in overrides]


def _grouped_test_names(config: dict[str, object]) -> set[str]:
    """Return exact test names assigned to the nested-Cargo test group."""
    filters = [
        str(override.get("filter", ""))
        for override in _default_overrides(config)
        if override.get("test-group") == CHILD_CARGO_GROUP
    ]
    return {
        name
        for filter_ in filters
        for name in re.findall(r"test\(=([a-z0-9_]+)\)", filter_)
    }


def _rust_test_sources() -> dict[Path, str]:
    """Return every Rust integration-test source keyed by repository path."""
    return {
        path: path.read_text(encoding="utf-8")
        for path in REPO_ROOT.joinpath("tests").rglob("*.rs")
    }


def _rust_functions(source: str) -> list[tuple[str, str, str, str]]:
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


def _cargo_wrappers(functions: list[tuple[str, str, str, str]]) -> set[str]:
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


def _build_capable_test_names(source: str) -> set[str]:
    """Return tests reaching a direct or helper-mediated Cargo build command."""
    functions = _rust_functions(source)
    wrappers = _cargo_wrappers(functions)
    operation_constants = _operation_constants(source)
    build_capable = {
        name
        for _, _, name, body in functions
        if _has_build_capable_cargo_command(body, wrappers, operation_constants)
        or _calls_build_helper(body)
    }

    while True:
        helper_names = {name for _, _, name, _ in functions if name in build_capable}
        callers = {
            name
            for _, _, name, body in functions
            if any(
                helper_name != "build" and re.search(rf"\b{helper_name}\s*\(", body)
                for helper_name in helper_names
            )
        }
        fixture_users = {
            name
            for attributes, signature, name, body in functions
            if "#[test]" in attributes or "#[rstest]" in attributes
            for helper_name in helper_names
            if any(
                "#[fixture]" in helper_attributes and helper_name in signature + body
                for helper_attributes, _, fixture_name, _ in functions
                if fixture_name == helper_name
            )
        }
        expanded = build_capable | callers | fixture_users
        if expanded == build_capable:
            break
        build_capable = expanded

    return {
        name
        for attributes, _, name, _ in functions
        if name in build_capable
        and ("#[test]" in attributes or "#[rstest]" in attributes)
    }


def test_nested_cargo_group_serializes_build_capable_tests() -> None:
    """The named group serializes every current nested Cargo build test."""
    config = _nextest_config()
    groups = require_mapping(config.get("test-groups"), "Nextest test groups")
    group = require_mapping(groups.get(CHILD_CARGO_GROUP), "nested Cargo test group")
    assert group.get("max-threads") == 1, (
        "nested Cargo builds must run one at a time on four-vCPU coverage runners"
    )
    grouped = _grouped_test_names(config)
    assert set(NESTED_CARGO_BUILD_TESTS) <= grouped, (
        f"nested Cargo build tests missing from {CHILD_CARGO_GROUP}: "
        f"{sorted(set(NESTED_CARGO_BUILD_TESTS) - grouped)!r}"
    )
    assert {
        "cli_configuration_fixture_compiles",
        "harness_compiles_under_a_split_build_dir",
        "packaged_manifest_retains_build_script_sources",
    } <= grouped, "the three known contended nested Cargo tests must be serialized"
    immediate = [
        override
        for override in _default_overrides(config)
        if override.get("test-group") == CHILD_CARGO_GROUP
    ]
    assert immediate, "nested Cargo build tests must have explicit overrides"
    assert all(
        override.get("success-output") == "immediate" for override in immediate
    ), "nested Cargo build diagnostics must remain immediate"


def test_nested_cargo_group_covers_discovered_build_commands() -> None:
    """Direct and helper-mediated build-capable Cargo commands remain grouped."""
    sources = _rust_test_sources()
    discovered = set().union(
        *(_build_capable_test_names(source) for source in sources.values())
    )
    assert discovered <= _grouped_test_names(_nextest_config()), (
        f"build-capable child Cargo tests missing group policy: "
        f"{sorted(discovered - _grouped_test_names(_nextest_config()))!r}"
    )


def test_direct_child_cargo_build_addition_requires_group_policy() -> None:
    """A new direct build-capable Cargo test is discovered before it can run."""
    source = """
#[rstest]
fn new_fixture_compiles() {
    Command::new(cargo()).arg("build");
}
"""
    assert _build_capable_test_names(source) == {"new_fixture_compiles"}, (
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
    assert _build_capable_test_names(source) == {
        "asynchronous_fixture_compiles",
        "restricted_fixture_compiles",
    }, "qualified and asynchronous build-capable tests must be discovered"


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
