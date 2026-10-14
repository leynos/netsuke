"""Classify Rust integration tests for the nested Cargo build contracts.

The ``nested-cargo-builds`` Nextest group only serializes a test whose name a
filter actually selects, so the contract tests need to know which tests exist
and which of them spawn a build-capable child Cargo command. That classification
lives here: parsing function attributes and signatures out of masked Rust
source, and propagating build capability from a command to its callers, its
transitive callers, and the tests that consume a helper as a fixture.

This is the Rust-source half of the pair. The Nextest-configuration half — the
filters and the accepted selector grammar — lives in
``nextest_child_cargo_group_invariants``, which re-exports the entry points
below so a contract test can import from either module.

Both halves are test-only: production code must not import either one.
"""

import re
import typing as typ

from rust_source_scan import mask_non_code
from workflow_loading import REPO_ROOT, WorkflowReadError

if typ.TYPE_CHECKING:
    from pathlib import Path

BUILD_CAPABLE_SUBCOMMANDS = ("build", "check", "package", "publish")
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
# A `#[case]` attribute, with or without a `::name`, in either the parenthesised
# or the bracketed spelling. It multiplies its test into instances at run time.
CASE_ATTRIBUTE = re.compile(r"#\[case(?:::[a-z0-9_]+)?[\(\[]")
RETAINED_RUST_LITERALS = {
    *(f'"{operation}"' for operation in BUILD_CAPABLE_SUBCOMMANDS),
    '"cargo"',
}
# A free function named `build` is reached as a bare `build()`. The `::` and `.`
# forms are excluded because an associated `Fixture::build()` is already traced
# by `_calls_build_helper`, and matching it here would attribute the call to an
# unrelated free function that happens to share the name.
BARE_BUILD_CALL = re.compile(r"(?<![.:\w])build\s*\(")
# The corpus is read from this directory unless a caller names another, so a
# probe can point the discovery at a scratch tree and confirm what it finds
# there without reaching into the production tree.
TEST_SOURCE_ROOT = REPO_ROOT / "tests"
type RustFunction = tuple[str, str, str, str]


def rust_test_sources(directory: Path | None = None) -> dict[Path, str]:
    """Return every Rust integration-test source keyed by repository path.

    The read is eager rather than left to a comprehension, so that the error
    names the path that caused it instead of the discovery that asked for it.
    A bare glob yields nothing for a path that is absent or is not a
    directory, so the reading refuses such a tree rather than returning an
    empty corpus: without that check every assertion above it would pass
    having read no test at all.

    Parameters
    ----------
    directory
        The tree to read; the repository's `tests` directory by default.

    Returns
    -------
    dict[Path, str]
        Each `**/*.rs` path under ``directory`` mapped to its text, in path
        order.

    Raises
    ------
    WorkflowReadError
        If a source is missing or does not decode as UTF-8, or if
        ``directory`` is absent or is not a directory. A contract that could
        not read a source cannot say anything about the tests it declares, so
        the failure is raised rather than left to surface as an empty corpus.
    """
    root = TEST_SOURCE_ROOT if directory is None else directory
    if not root.is_dir():
        message = f"{root} is not a directory, so no test source was read"
        raise WorkflowReadError(message)
    sources: dict[Path, str] = {}
    for path in sorted(root.rglob("*.rs")):
        try:
            sources[path] = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            message = f"{path} could not be read: {error}"
            raise WorkflowReadError(message) from error
    return sources


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
        re.search(rf"\b{re.escape(wrapper)}\s*\(", body) for wrapper in wrappers
    )
    operation_supplied = bool(CARGO_OPERATION.search(body)) or any(
        re.search(rf"\b{re.escape(constant)}\b", body)
        for constant in operation_constants
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


def _caller_pattern(helper_name: str) -> re.Pattern[str]:
    """Return the pattern that matches a call to `helper_name` from a body."""
    if helper_name == "build":
        return BARE_BUILD_CALL
    return re.compile(rf"\b{re.escape(helper_name)}\s*\(")


def _callers_of_build_capable_helpers(
    functions: list[RustFunction], helper_names: set[str]
) -> set[str]:
    """Return functions that call a build-capable helper by its bare name.

    A helper named `build` is matched only as a bare call, because the
    associated `Fixture::build()` spelling belongs to `_calls_build_helper`.

    Returns
    -------
    set[str]
        The names of functions whose body calls a helper by its bare name.
    """
    patterns = [_caller_pattern(helper_name) for helper_name in helper_names]
    return {
        name
        for _, _, name, body in functions
        if any(pattern.search(body) for pattern in patterns)
    }


def _fixture_users_of_build_capable_helpers(
    functions: list[RustFunction], helper_names: set[str]
) -> set[str]:
    """Return Rust tests that consume a build-capable fixture helper.

    Each fixture name is matched as a whole identifier. A bare substring test
    would let `prepare_build` count as a use of a fixture named `build`, so a
    test would inherit build capability from a helper it never named.

    Returns
    -------
    set[str]
        The names of tests naming a build-capable fixture in their signature
        or body.
    """
    fixtures = {
        name
        for attributes, _, name, _ in functions
        if "#[fixture]" in attributes and name in helper_names
    }
    return {
        name
        for attributes, signature, name, body in functions
        if _is_rust_test(attributes)
        and any(
            re.search(rf"\b{re.escape(fixture)}\b", signature + body)
            for fixture in fixtures
        )
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
    """Return tests reaching a direct or helper-mediated Cargo build command.

    Capability is propagated to a fixed point before the result is narrowed:
    it reaches callers and fixture users transitively, so a helper that is
    itself build-capable is not reported unless it is also a test.

    Parameters
    ----------
    source
        One Rust integration-test source, read as text. It is masked before
        parsing, so comments and string literals cannot be mistaken for code.

    Returns
    -------
    set[str]
        The declared tests whose bodies reach a build-capable Cargo command,
        directly or through intermediate helpers.
    """
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


def declared_test_names(directory: Path | None = None) -> dict[str, set[str]]:
    """Return every declared test name mapped to the sources declaring it.

    Parameters
    ----------
    directory
        The tree to read; the repository's `tests` directory by default, and
        forwarded to `rust_test_sources` so the read boundary stays in one
        place rather than being reached ambiently from a second caller. The
        read is fallible in the ways `rust_test_sources` documents.

    Returns
    -------
    dict[str, set[str]]
        Each declared test name to the repository paths of the sources that
        declare it.
    """
    declared: dict[str, set[str]] = {}
    for path, source in rust_test_sources(directory).items():
        executable_source = mask_non_code(source, RETAINED_RUST_LITERALS)
        for attributes, _, name, _ in _rust_functions(executable_source):
            if _is_rust_test(attributes):
                declared.setdefault(name, set()).add(str(path))
    return declared
