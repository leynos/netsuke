//! Tests for the build audit's Fluent and Cargo-metadata parsers.
//!
//! Both live in the build script, which `cargo test` does not build as a test
//! target, so the modules are included here by path. Neither depends on the
//! library crate, so each compiles standalone.

#[path = "../build_l10n_audit/ftl.rs"]
mod ftl;
#[path = "../build_l10n_audit/metadata.rs"]
mod metadata;

use std::collections::BTreeSet;
use std::io::Write as _;

use anyhow::{Result, anyhow, bail, ensure};
use rstest::rstest;
use tempfile::NamedTempFile;

// ---------------------------------------------------------------- FTL parser

/// Stage `source` as a catalogue and parse it.
fn parse(source: &str) -> Result<ftl::MessageVariables> {
    let mut file = NamedTempFile::new()?;
    file.write_all(source.as_bytes())?;
    file.flush()?;
    ftl::parse_catalogue(file.path()).map_err(|error| anyhow!("{error}"))
}

/// The variables the parser found for `key`.
fn variables_of(parsed: &ftl::MessageVariables, key: &str) -> Result<Vec<String>> {
    let found = parsed
        .get(key)
        .ok_or_else(|| anyhow!("{key} was not parsed; got {:?}", parsed.keys()))?;
    Ok(found.iter().cloned().collect())
}

fn names(expected: &[&str]) -> Vec<String> {
    expected.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn message_identifiers_are_collected() -> Result<()> {
    let parsed = parse("first = one\nsecond = two\n")?;
    let ids: BTreeSet<&str> = parsed.keys().map(String::as_str).collect();
    ensure!(
        ids == BTreeSet::from(["first", "second"]),
        "expected both identifiers, got {ids:?}"
    );
    Ok(())
}

/// Comments carry translator context, including `$` examples, and must not
/// contribute either identifiers or variables.
#[rstest]
#[case("# a comment mentioning { $ghost }\nkey = value\n")]
#[case("  # an indented comment with { $ghost }\nkey = value\n")]
#[case("## a group comment { $ghost }\nkey = value\n")]
fn comments_contribute_nothing(#[case] source: &str) -> Result<()> {
    let parsed = parse(source)?;
    let ids: BTreeSet<&str> = parsed.keys().map(String::as_str).collect();
    ensure!(ids == BTreeSet::from(["key"]), "got {ids:?}");
    ensure!(
        variables_of(&parsed, "key")?.is_empty(),
        "a comment leaked a variable into the message"
    );
    Ok(())
}

#[rstest]
// One variable on the value line.
#[case("key = uses { $path }\n", &["path"])]
// Several, including a repeat, which the set collapses.
#[case("key = { $min } to { $max } and { $min }\n", &["max", "min"])]
// A variable on an indented continuation line.
#[case("key = opening\n    continued with { $detail }\n", &["detail"])]
// A `select` expression: the selector and the variants both count.
#[case(
    "key = { $count ->\n    [one] one { $item }\n   *[other] many { $item }\n}\n",
    &["count", "item"]
)]
// Underscores and digits are part of a variable name; a bare `$` is not.
#[case("key = { $task_progress } and $ alone\n", &["task_progress"])]
fn variables_are_collected(#[case] source: &str, #[case] expected: &[&str]) -> Result<()> {
    let parsed = parse(source)?;
    let found = variables_of(&parsed, "key")?;
    ensure!(
        found == names(expected),
        "expected {expected:?}, got {found:?}"
    );
    Ok(())
}

/// A blank line ends a message body, so an indented line after one belongs to
/// no message and its variables must not be attributed to the message above.
#[test]
fn a_blank_line_ends_the_message_body() -> Result<()> {
    let parsed = parse("key = value\n\n    stray { $ghost }\nother = second\n")?;
    ensure!(
        variables_of(&parsed, "key")?.is_empty(),
        "a line after a blank line was attributed to the message above it"
    );
    Ok(())
}

/// Terms are reusable fragments that code never references, so the audit
/// ignores them rather than demanding a matching key.
#[test]
fn terms_are_ignored() -> Result<()> {
    let parsed = parse("-brand = Netsuke\nkey = value\n")?;
    let ids: BTreeSet<&str> = parsed.keys().map(String::as_str).collect();
    ensure!(ids == BTreeSet::from(["key"]), "got {ids:?}");
    Ok(())
}

/// An empty catalogue is a mistake worth failing on: it would make every
/// declared key look missing.
#[rstest]
#[case("")]
#[case("# only comments\n")]
fn catalogues_without_messages_are_rejected(#[case] source: &str) -> Result<()> {
    let mut file = NamedTempFile::new()?;
    file.write_all(source.as_bytes())?;
    file.flush()?;
    match ftl::parse_catalogue(file.path()) {
        Ok(parsed) => bail!("expected a parse failure, got {parsed:?}"),
        Err(error) => ensure!(
            error.to_string().contains("no Fluent messages found"),
            "unexpected error: {error}"
        ),
    }
    Ok(())
}

// ----------------------------------------------------------- Cargo metadata

fn metadata_locales(manifest: &str) -> Option<Vec<&str>> {
    metadata::parse_metadata_locales(manifest)
}

const TABLE: &str = "[package.metadata.ortho_config]\nroot_type = \"x\"\n";

#[test]
fn a_single_line_array_is_read() -> Result<()> {
    let manifest = format!("{TABLE}locales = [\"en-US\", \"fr\"]\n");
    let found = metadata_locales(&manifest)
        .ok_or_else(|| anyhow!("expected the locales key to be found"))?;
    ensure!(found == ["en-US", "fr"], "got {found:?}");
    Ok(())
}

#[test]
fn a_multiline_array_is_read() -> Result<()> {
    let manifest = format!("{TABLE}locales   = [\n    \"en-US\",\n    \"fr\",\n]\n\n[features]\n");
    let found = metadata_locales(&manifest)
        .ok_or_else(|| anyhow!("expected the locales key to be found"))?;
    ensure!(found == ["en-US", "fr"], "got {found:?}");
    Ok(())
}

/// The key must be matched as a whole assignment: a comment mentioning
/// locales, or a neighbouring key that merely ends in `locales`, must not be
/// read in its place.
#[rstest]
#[case("# locales = [\"wrong\"]\nlocales = [\"en-US\"]\n")]
#[case("extra_locales = [\"wrong\"]\nlocales = [\"en-US\"]\n")]
#[case("locales_note = \"see [wrong]\"\nlocales = [\"en-US\"]\n")]
fn decoys_do_not_displace_the_key(#[case] body: &str) -> Result<()> {
    let manifest = format!("{TABLE}{body}");
    let found = metadata_locales(&manifest)
        .ok_or_else(|| anyhow!("expected the locales key to be found"))?;
    ensure!(found == ["en-US"], "got {found:?}");
    Ok(())
}

#[rstest]
// No such table.
#[case("[package]\nname = \"netsuke\"\n")]
// The table, but no locales key.
#[case("[package.metadata.ortho_config]\nroot_type = \"x\"\n")]
// The key appears only after the table has ended.
#[case("[package.metadata.ortho_config]\nroot_type = \"x\"\n\n[other]\nlocales = [\"en-US\"]\n")]
// An unterminated array.
#[case("[package.metadata.ortho_config]\nlocales = [\"en-US\",\n")]
fn absent_or_unterminated_keys_yield_none(#[case] manifest: &str) -> Result<()> {
    ensure!(
        metadata_locales(manifest).is_none(),
        "expected no locales, got {:?}",
        metadata_locales(manifest)
    );
    Ok(())
}

/// The repository manifest is the input this parser exists to read.
#[test]
fn the_repository_manifest_parses() -> Result<()> {
    let found = metadata_locales(include_str!("../Cargo.toml"))
        .ok_or_else(|| anyhow!("expected the repository manifest to declare locales"))?;
    ensure!(
        found.contains(&"en-US") && found.contains(&"zh-Hant"),
        "expected the repository locales, got {found:?}"
    );
    Ok(())
}
