"""Protect nested-Cargo discovery from non-code Rust syntax."""

import pytest
from nextest_child_cargo_group_invariants import build_capable_test_names as _build_capable_test_names


def test_build_capable_discovery_ignores_block_comment_functions() -> None:
    """Function-like text in a block comment does not create a nested Cargo test."""
    source = """
/*
#[test]
fn fake_fixture_compiles() {
    Command::new(cargo()).arg("build");
}
*/
"""
    assert not _build_capable_test_names(source), (
        "block comments must not create build-capable child Cargo tests"
    )


@pytest.mark.parametrize("literal_prefix", ["/*", 'r#"'])
def test_non_code_function_text_does_not_truncate_real_test(
    literal_prefix: str,
) -> None:
    """Comments and raw strings cannot hide a real child Cargo command."""
    literal_suffix = "*/" if literal_prefix == "/*" else '"#'
    source = f"""
#[test]
fn real_fixture_compiles() {{
    let non_code = {literal_prefix}
#[test]
fn fake_fixture_compiles() {{
    Command::new(cargo()).arg("build");
}}
{literal_suffix};
    Command::new(cargo()).arg("build");
}}
"""
    assert _build_capable_test_names(source) == {"real_fixture_compiles"}, (
        "non-code function text must not truncate the real test body"
    )


@pytest.mark.parametrize(
    "prefix",
    ["<'a>", "() { 'outer: loop { break 'outer; }"],
)
def test_lifetime_and_label_syntax_do_not_hide_child_cargo_commands(
    prefix: str,
) -> None:
    """Lifetimes and labels remain executable source, not character literals."""
    source = f"""
#[test]
fn real_fixture_compiles{prefix} {{
    Command::new(cargo()).arg("build");
}}
"""
    assert _build_capable_test_names(source) == {"real_fixture_compiles"}, (
        "lifetimes and labels must not mask child Cargo commands"
    )


def test_character_literals_remain_masked() -> None:
    """A valid escaped character literal does not disturb child Cargo discovery."""
    source = """
#[test]
fn real_fixture_compiles() {
    let newline = '\\n';
    Command::new(cargo()).arg("build");
}
"""
    assert _build_capable_test_names(source) == {"real_fixture_compiles"}, (
        "character literals must not hide child Cargo commands"
    )
