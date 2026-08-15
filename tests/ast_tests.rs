//! Unit tests for Netsuke manifest AST deserialization.
//!
//! The cases are split by manifest domain across `tests/ast_tests/` so no file
//! exceeds the repository's 400-line limit.

#[path = "ast_tests/actions.rs"]
mod actions;
#[path = "ast_tests/macros.rs"]
mod macros;
#[path = "ast_tests/manifest_files.rs"]
mod manifest_files;
#[path = "ast_tests/parsing.rs"]
mod parsing;
#[path = "ast_tests/recipe.rs"]
mod recipe;
#[path = "ast_tests/string_or_list.rs"]
mod string_or_list;
#[path = "ast_tests/support.rs"]
mod support;
