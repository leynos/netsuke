//! Tests for the `define_keys!` parser used by the build-time localization
//! audit.
//!
//! The parser lives in the build script, which `cargo test` does not build as
//! a test target, so the module is included here by path. Only
//! `extract_key_constants` is reachable, which is the surface `build.rs` uses;
//! the scanner is exercised through it.

#[path = "../build_l10n_audit/keys.rs"]
mod keys;

use std::collections::BTreeSet;
use std::io::Write as _;

use anyhow::{Result, anyhow, bail, ensure};
use rstest::rstest;
use tempfile::NamedTempFile;

/// Stage `source` as a Rust file for the extractor to read.
fn write_source(source: &str) -> Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    file.write_all(source.as_bytes())?;
    file.flush()?;
    Ok(file)
}

/// Wrap `entries` in a `define_keys!` invocation and extract its keys.
fn extract(entries: &str) -> Result<BTreeSet<String>> {
    extract_source(&format!("define_keys! {{\n{entries}\n}}\n"))
}

fn extract_source(source: &str) -> Result<BTreeSet<String>> {
    let file = write_source(source)?;
    keys::extract_key_constants(file.path()).map_err(|error| anyhow!("{error}"))
}

/// Extract from a `define_keys!` body expected to fail, returning the message.
fn extraction_error(entries: &str) -> Result<String> {
    let source = format!("define_keys! {{\n{entries}\n}}\n");
    let file = write_source(&source)?;
    match keys::extract_key_constants(file.path()) {
        Ok(extracted) => bail!("expected extraction to fail, got {extracted:?}"),
        Err(error) => Ok(error.to_string()),
    }
}

fn key_set(keys: &[&str]) -> BTreeSet<String> {
    keys.iter().map(|key| (*key).to_owned()).collect()
}

#[rstest]
// A plain entry, the shape the real macro uses.
#[case("CLI_ABOUT => \"cli.about\",", &["cli.about"])]
// Several entries, including one whose value escapes a quote.
#[case(
    "A => \"first.key\",\n    B => \"second.key\",",
    &["first.key", "second.key"]
)]
#[case(r#"A => "quoted\"key","#, &["quoted\"key"])]
// A backslash escape other than a quote keeps the escaped character.
#[case(r#"A => "back\\slash","#, &["back\\slash"])]
fn regular_string_literals_yield_their_keys(
    #[case] entries: &str,
    #[case] expected: &[&str],
) -> Result<()> {
    let extracted = extract(entries)?;
    ensure!(
        extracted == key_set(expected),
        "expected {expected:?}, got {extracted:?}"
    );
    Ok(())
}

#[rstest]
// A raw string with no hashes.
#[case("A => r\"raw.key\",", &["raw.key"])]
// One hash, so the value may contain a quote.
#[case("A => r#\"raw \"quoted\" key\"#,", &["raw \"quoted\" key"])]
// Two hashes, so the value may contain a quote-hash pair.
#[case("A => r##\"raw \"# key\"##,", &["raw \"# key"])]
// A raw string does not process escapes.
#[case("A => r\"back\\slash\",", &["back\\slash"])]
fn raw_string_literals_yield_their_keys(
    #[case] entries: &str,
    #[case] expected: &[&str],
) -> Result<()> {
    let extracted = extract(entries)?;
    ensure!(
        extracted == key_set(expected),
        "expected {expected:?}, got {extracted:?}"
    );
    Ok(())
}

/// Commented-out entries must not contribute keys, or a key removed by
/// commenting it out would still be demanded of every catalogue.
#[rstest]
#[case(
    "A => \"live.key\",\n    // B => \"commented.key\",",
    &["live.key"]
)]
#[case(
    "A => \"live.key\",\n    /* B => \"commented.key\", */",
    &["live.key"]
)]
// A block comment spanning lines.
#[case(
    "A => \"live.key\",\n    /*\n    B => \"commented.key\",\n    */",
    &["live.key"]
)]
// A line comment as the final line, with no trailing newline inside the body.
#[case("A => \"live.key\", // trailing", &["live.key"])]
fn comments_are_skipped(#[case] entries: &str, #[case] expected: &[&str]) -> Result<()> {
    let extracted = extract(entries)?;
    ensure!(
        extracted == key_set(expected),
        "expected {expected:?}, got {extracted:?}"
    );
    Ok(())
}

#[rstest]
// An unterminated regular string.
#[case("A => \"unterminated,", "unterminated string literal")]
// An unterminated raw string.
#[case("A => r#\"unterminated,", "unterminated raw string literal")]
// A raw marker with no opening quote.
#[case("A => r#x\",", "raw string literal missing opening quote")]
// A value that is not a string literal at all.
#[case("A => 42,", "expected string literal after define_keys! =>")]
fn malformed_literals_are_rejected(#[case] entries: &str, #[case] expected: &str) -> Result<()> {
    let message = extraction_error(entries)?;
    ensure!(
        message.contains(expected),
        "expected an error mentioning {expected:?}, got {message:?}"
    );
    Ok(())
}

/// Byte strings carry bytes rather than text, so they cannot name a Fluent
/// message. A raw byte string is reported specifically; a plain one fails
/// earlier, when the mandatory `r` marker is found missing.
#[rstest]
#[case("A => br\"bytes\",", "byte string literals are not supported")]
#[case("A => br#\"bytes\"#,", "byte string literals are not supported")]
#[case("A => b\"bytes\",", "expected string literal after define_keys! =>")]
fn byte_string_literals_are_rejected(#[case] entries: &str, #[case] expected: &str) -> Result<()> {
    let message = extraction_error(entries)?;
    ensure!(
        message.contains(expected),
        "expected an error mentioning {expected:?}, got {message:?}"
    );
    Ok(())
}

#[rstest]
// No macro at all.
#[case("fn main() {}\n", "define_keys! macro not found")]
// The macro name present but no body.
#[case("define_keys!\n", "define_keys! macro body is missing '{'")]
// An unclosed body.
#[case(
    "define_keys! {\n    A => \"a\",\n",
    "define_keys! macro body is missing '}'"
)]
// A body with no entries.
#[case("define_keys! {\n}\n", "no localization keys found")]
fn malformed_macro_invocations_are_rejected(
    #[case] source: &str,
    #[case] expected: &str,
) -> Result<()> {
    let file = write_source(source)?;
    let message = match keys::extract_key_constants(file.path()) {
        Ok(extracted) => bail!("expected extraction to fail, got {extracted:?}"),
        Err(error) => error.to_string(),
    };
    ensure!(
        message.contains(expected),
        "expected an error mentioning {expected:?}, got {message:?}"
    );
    Ok(())
}

/// Braces inside comments and string literals are not source, so the body
/// scan must step over them. Before it did, a doc comment mentioning `}` or a
/// key whose value contained one truncated the declared key set or failed the
/// build outright.
#[rstest]
// An unbalanced brace in a line comment.
#[case("A => \"a.key\",\n    // { stray brace", "a.key")]
// An unbalanced brace in a doc comment, which is how this would really arrive.
#[case("/// Braces like } appear in prose.\n    A => \"a.key\",", "a.key")]
// An unbalanced brace in a block comment.
#[case("/* } */\n    A => \"a.key\",", "a.key")]
// An unbalanced brace inside a key's value.
#[case("A => \"{\",", "{")]
// An unbalanced brace inside a raw string value.
#[case("A => r#\"a}key\"#,", "a}key")]
fn braces_in_comments_and_literals_do_not_end_the_body(
    #[case] entries: &str,
    #[case] expected: &str,
) -> Result<()> {
    let extracted = extract(entries)?;
    ensure!(
        extracted.contains(expected),
        "expected the body scan to reach {expected:?}, got {extracted:?}"
    );
    Ok(())
}

/// A body that genuinely never closes is still an error.
#[test]
fn an_unterminated_body_is_still_reported() -> Result<()> {
    let source = "define_keys! {\n    A => \"a.key\",\n";
    let message = match extract_source(source) {
        Ok(keys) => bail!("expected an unterminated body error, got {keys:?}"),
        Err(error) => error.to_string(),
    };
    ensure!(
        message.contains("define_keys! macro body is missing '}'"),
        "expected the body scan to report an unterminated body, got {message:?}"
    );
    Ok(())
}

/// Macro-shaped text that is not the invocation must not be mistaken for it.
///
/// A doc comment naming `define_keys!`, or a string containing it, previously
/// captured the search: extraction started from the mention and read the wrong
/// text as the body.
#[rstest]
// Named in a line doc comment above the real invocation.
#[case("/// See define_keys! for the declaration format.\n")]
// Named in a block comment.
#[case("/* define_keys! { NOT => \"not.a.key\", } */\n")]
// Quoted in a string constant, braces and all.
#[case("const SAMPLE: &str = \"define_keys! { NOT => \\\"not.a.key\\\", }\";\n")]
fn a_mention_of_the_macro_does_not_displace_the_invocation(#[case] preamble: &str) -> Result<()> {
    let source = format!("{preamble}define_keys! {{\n    A => \"a.key\",\n}}\n");
    let extracted = extract_source(&source)?;
    ensure!(
        extracted == key_set(&["a.key"]),
        "expected only the real invocation's key, got {extracted:?}"
    );
    Ok(())
}

/// Trivia between the macro name and its delimiter must not be mistaken for
/// the delimiter, and nested block comments must close at the outer `*/`.
#[rstest]
// A brace inside a comment between the name and the real delimiter.
#[case("define_keys! /* { */ {\n    A => \"a.key\",\n}\n")]
// A line comment between the name and the delimiter.
#[case("define_keys! // opens below\n{\n    A => \"a.key\",\n}\n")]
// A nested block comment before the invocation.
#[case("/* outer /* inner */ still a comment */\ndefine_keys! {\n    A => \"a.key\",\n}\n")]
// A macro whose name merely ends with the one being looked for.
#[case("other_define_keys! { NOT => \"not.a.key\", }\ndefine_keys! {\n    A => \"a.key\",\n}\n")]
fn the_real_invocation_is_selected(#[case] source: &str) -> Result<()> {
    let extracted = extract_source(source)?;
    ensure!(
        extracted == key_set(&["a.key"]),
        "expected only the real invocation's key, got {extracted:?}"
    );
    Ok(())
}

/// The real macro body is the contract this parser exists to read.
#[test]
fn the_repository_macro_parses() -> Result<()> {
    let extracted = extract_source(include_str!("../src/localization/keys.rs"))?;
    ensure!(
        extracted.contains("cli.about"),
        "expected the repository keys to include cli.about"
    );
    ensure!(
        extracted.len() > 300,
        "expected the repository to declare over 300 keys, got {}",
        extracted.len()
    );
    Ok(())
}
