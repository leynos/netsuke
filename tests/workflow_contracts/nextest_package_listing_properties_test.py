"""Property-check build discovery against Cargo's package-list semantics.

The fixed examples in ``nextest_child_cargo_syntax_test`` pin readable Rust
spellings. This module varies argument order, spelling, and statement tails to
check the same rule against an independent model: only an exact ``-l`` or
``--list`` argument to ``package`` makes it a non-building file listing.

Run via ``make test-workflow-contracts``.
"""

from hypothesis import example, given
from hypothesis import strategies as st
from nextest_child_cargo_group_invariants import (
    build_capable_test_names as _build_capable_test_names,
)

BUILD_CAPABLE_OPERATIONS = ("build", "check", "test", "package", "publish")
PACKAGE_LIST_FLAGS = frozenset(("-l", "--list"))
ARGUMENT_ATOMS = (
    "-l",
    "--list",
    "-lfile",
    "--list-files",
    "--list=true",
    "-p",
    "netsuke-build",
    "--allow-dirty",
)
ARGUMENT_TAILS = st.lists(st.sampled_from(ARGUMENT_ATOMS), max_size=5).map(tuple)
INVOCATIONS = st.one_of(
    st.tuples(st.just("arg"), st.just(())),
    st.tuples(st.just("args"), ARGUMENT_TAILS),
    st.tuples(st.just("chained"), ARGUMENT_TAILS),
    st.tuples(st.just("constant"), ARGUMENT_TAILS),
)


@example(
    operation="package",
    invocation=("args", ("-p", "netsuke-build", "--list")),
    unrelated_after_statement=None,
)
@example(
    operation="package",
    invocation=("chained", ("--allow-dirty", "-l")),
    unrelated_after_statement=None,
)
@example(
    operation="package",
    invocation=("args", ("--list-files",)),
    unrelated_after_statement="--list",
)
@example(
    operation="build",
    invocation=("args", ("-l",)),
    unrelated_after_statement=None,
)
@given(
    operation=st.sampled_from(BUILD_CAPABLE_OPERATIONS),
    invocation=INVOCATIONS,
    unrelated_after_statement=st.one_of(st.none(), st.sampled_from(ARGUMENT_ATOMS)),
)
def test_discovery_matches_package_listing_model_across_argument_forms(
    operation: str,
    invocation: tuple[str, tuple[str, ...]],
    unrelated_after_statement: str | None,
) -> None:
    """Classify Cargo commands from generated arguments and supported spellings."""
    spelling, trailing_arguments = invocation
    arguments = (operation, *trailing_arguments)
    rendered_arguments = ", ".join(f'"{argument}"' for argument in arguments)

    match spelling:
        case "arg":
            command = f'Command::new(cargo()).arg("{operation}");'
            constant = ""
        case "args":
            command = f"Command::new(cargo()).args([{rendered_arguments}]);"
            constant = ""
        case "chained":
            calls = "".join(f'.arg("{argument}")' for argument in arguments)
            command = f"Command::new(cargo()){calls};"
            constant = ""
        case "constant":
            constant = f"const CARGO_ARGUMENTS: &[&str] = &[{rendered_arguments}];"
            command = "Command::new(cargo()).args(CARGO_ARGUMENTS);"

    unrelated = (
        f'let unrelated = "{unrelated_after_statement}";'
        if unrelated_after_statement is not None
        else ""
    )
    source = f"""
{constant}
#[test]
fn cargo_operation() {{
    {command}
    {unrelated}
}}
"""

    expected: set[str] = (
        {"cargo_operation"}
        if operation != "package"
        or not PACKAGE_LIST_FLAGS.intersection(trailing_arguments)
        else set()
    )
    assert _build_capable_test_names(source) == expected, (
        "package listing depends on exact list flags within its command "
        "statement; other Cargo build operations remain build-capable"
    )
