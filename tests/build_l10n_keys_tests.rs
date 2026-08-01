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

/// The body scanner counts braces without regard for comments or string
/// literals, so an unbalanced brace inside either makes it run past the
/// macro's closing brace and report the body as unterminated.
///
/// The repository's own macro contains no such brace, and the audit fails
/// loudly rather than silently dropping keys, so the behaviour is pinned here
/// rather than relied upon. A parser that tracked comments and strings in this
/// pass would be an improvement, not a bug fix.
#[rstest]
// A brace inside a line comment.
#[case("A => \"a.key\",\n    // { stray brace")]
// A brace inside a key's value.
#[case("A => \"{\",")]
fn unbalanced_braces_in_comments_or_values_end_the_body_early(#[case] entries: &str) -> Result<()> {
    let message = extraction_error(entries)?;
    ensure!(
        message.contains("define_keys! macro body is missing '}'"),
        "expected the body scan to report an unterminated body, got {message:?}"
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
