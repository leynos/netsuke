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

use anyhow::{Result, anyhow, bail, ensure};
use rstest::rstest;

// ---------------------------------------------------------------- FTL parser

/// Parse `source` as a catalogue.
fn parse(source: &str) -> Result<ftl::MessageVariables> {
    ftl::parse_catalogue(source).map_err(|error| anyhow!("{error}"))
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
///
/// Both cases begin a line, which is what makes them comments. An indented
/// `#` is a continuation instead, whatever it looks like; see
/// [`an_indented_hash_line_continues_the_message`] and
/// [`an_indented_line_before_any_message_contributes_nothing`].
#[rstest]
#[case("# a comment mentioning { $ghost }\nkey = value\n")]
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

/// An indented line with no message above it has nothing to continue.
///
/// It reaches the continuation branch rather than the comment branch — the
/// parser tests indentation first — and is dropped for want of a message to
/// attach to. The outcome matches a comment's, but the route does not, so it
/// is asserted separately rather than filed among the comment cases where it
/// would look like evidence the comment branch had run.
#[test]
fn an_indented_line_before_any_message_contributes_nothing() -> Result<()> {
    let parsed = parse("  # an indented line with { $ghost }\nkey = value\n")?;
    let ids: BTreeSet<&str> = parsed.keys().map(String::as_str).collect();
    ensure!(ids == BTreeSet::from(["key"]), "got {ids:?}");
    ensure!(
        variables_of(&parsed, "key")?.is_empty(),
        "an orphaned continuation leaked a variable into the next message"
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

/// A blank line does not end a pattern; the next entry does.
///
/// This previously asserted the opposite. Fluent permits blank lines inside a
/// multiline pattern, so an indented line after one is still a continuation of
/// the message above, and its variables belong to that message. Reading it the
/// old way dropped variables the audit is meant to compare.
#[test]
fn an_indented_line_after_a_blank_continues_the_message() -> Result<()> {
    let parsed = parse("key = value\n\n    continued { $ghost }\nother = second\n")?;
    ensure!(
        variables_of(&parsed, "key")? == ["ghost"],
        "the continuation's variable belongs to the message above the blank line"
    );
    ensure!(
        variables_of(&parsed, "other")?.is_empty(),
        "the next entry starts a new message"
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
    match ftl::parse_catalogue(source) {
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

/// The table header must be matched only where it begins a line. A commented
/// or quoted mention of it earlier in the manifest previously captured the
/// search, so the parser returned the text above the real table and the audit
/// reported a valid manifest as missing its metadata.
#[rstest]
// A commented-out header before the real one.
#[case("# [package.metadata.ortho_config]\n# locales = [\"wrong\"]\n")]
// The header named inside a string value.
#[case("[package]\ndescription = \"see [package.metadata.ortho_config]\"\n")]
// Indented, so `begins_a_line` sees a preceding fragment that trims to empty
// and must still accept it as a real header.
#[case("[package]\nname = \"netsuke\"\n  ")]
fn a_decoy_header_does_not_displace_the_table(#[case] preamble: &str) -> Result<()> {
    let manifest = format!("{preamble}{TABLE}locales = [\"en-US\"]\n");
    let found = metadata_locales(&manifest)
        .ok_or_else(|| anyhow!("expected the locales key to be found"))?;
    ensure!(found == ["en-US"], "got {found:?}");
    Ok(())
}

/// A multiline string can contain a line that looks like the table header or
/// the key. Its content is not source, so it must not capture the search.
#[rstest]
// The header spelled inside a multiline basic string, at column zero.
#[case(
    "[package]\ndescription = \"\"\"\n[package.metadata.ortho_config]\nlocales = [\"wrong\"]\n\"\"\"\n"
)]
// The same, in a multiline literal string.
#[case(
    "[package]\ndescription = '''\n[package.metadata.ortho_config]\nlocales = [\"wrong\"]\n'''\n"
)]
fn a_multiline_string_decoy_does_not_displace_the_table(#[case] preamble: &str) -> Result<()> {
    let manifest = format!("{preamble}{TABLE}locales = [\"en-US\"]\n");
    let found = metadata_locales(&manifest)
        .ok_or_else(|| anyhow!("expected the real locales key to be found"))?;
    ensure!(found == ["en-US"], "got {found:?}");
    Ok(())
}

/// A multiline string *inside* the table body can contain a line beginning
/// with `[`. That line is content, not the next table header, so it must not
/// end the table early and hide a `locales` key declared after it.
#[rstest]
// A header-shaped line inside a multiline basic string, before the key.
#[case("note = \"\"\"\n[not.a.header]\n\"\"\"\nlocales = [\"en-US\"]\n\n[features]\n")]
// The same, in a multiline literal string.
#[case("note = '''\n[not.a.header]\n'''\nlocales = [\"en-US\"]\n\n[features]\n")]
fn a_bracket_inside_a_table_string_does_not_end_the_table(#[case] body: &str) -> Result<()> {
    let manifest = format!("{TABLE}{body}");
    let found = metadata_locales(&manifest)
        .ok_or_else(|| anyhow!("expected the locales key inside the table to be found"))?;
    ensure!(found == ["en-US"], "got {found:?}");
    Ok(())
}

/// A nested array value can open a line with `[` inside a multiline array.
///
/// That line is a value, not the next table header, so the scan must continue
/// past it rather than truncating the table above the key that follows.
#[rstest]
// A nested string array before the key.
#[case("note = [\n[\"decoy\"],\n]\nlocales = [\"en-US\"]\n\n[features]\n")]
// The same with the nested value unterminated by a comma, as a final element.
#[case("note = [\n[\"decoy\"]\n]\nlocales = [\"en-US\"]\n\n[features]\n")]
fn a_nested_array_value_does_not_end_the_table(#[case] body: &str) -> Result<()> {
    let manifest = format!("{TABLE}{body}");
    let found = metadata_locales(&manifest)
        .ok_or_else(|| anyhow!("expected the locales key after the nested array to be found"))?;
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
// A quoted key names a table as legally as a bare one, so a quoted header
// ends the table too; reading past it would return the next table's keys.
#[case("[package.metadata.ortho_config]\n[\"release metadata\"]\nlocales = [\"en-US\"]\n")]
// The same, quoting one segment of a dotted header.
#[case("[package.metadata.ortho_config]\n[tool.\"release metadata\"]\nlocales = [\"en-US\"]\n")]
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

/// Fluent allows a blank line inside a multiline pattern; it does not end the
/// pattern. Clearing the current message there would drop the variables of
/// every continuation after it.
#[test]
fn a_blank_line_does_not_end_a_pattern() -> Result<()> {
    let parsed =
        parse("a.key = first { $one }\n    continued { $two }\n\n    after blank { $three }\n")?;
    let found = variables_of(&parsed, "a.key")?;
    ensure!(
        found == ["one", "three", "two"],
        "expected all three variables, got {found:?}"
    );
    Ok(())
}

/// Only U+0020 indents a continuation. A tab-indented line is not one, so its
/// variables must not be attributed to the message above.
#[test]
fn a_tab_indented_line_is_not_a_continuation() -> Result<()> {
    let parsed = parse("a.key = first { $one }\n\tb.key = tabbed { $two }\n")?;
    let found = variables_of(&parsed, "a.key")?;
    ensure!(
        found == ["one"],
        "expected only $one on a.key, got {found:?}"
    );
    Ok(())
}

/// An indented continuation beginning with `#` is pattern text, not a comment.
///
/// Fluent's comment syntax applies only to entry-starting lines, so a
/// continuation keeps its text whatever its first character. Reading it as a
/// comment dropped the variables it referenced — and dropping a variable makes
/// the audit *less* likely to complain, so it would have failed silently.
#[test]
fn an_indented_hash_line_continues_the_message() -> Result<()> {
    let parsed =
        parse("a.key = first { $one }\n    #{ $two } still the pattern\nother = second\n")?;
    let found = variables_of(&parsed, "a.key")?;
    ensure!(
        found == ["one", "two"],
        "expected both variables, got {found:?}"
    );
    // The continuation must not swallow the next entry: `other` starts a new
    // message of its own, with nothing inherited from the pattern above.
    ensure!(
        variables_of(&parsed, "other")?.is_empty(),
        "the next entry must start a fresh message with no variables"
    );
    Ok(())
}

/// An unindented comment is still a comment.
#[test]
fn an_unindented_hash_line_is_a_comment() -> Result<()> {
    let parsed = parse("a.key = first { $one }\n# { $ghost } a comment\nb.key = second\n")?;
    let found = variables_of(&parsed, "a.key")?;
    ensure!(
        found == ["one"],
        "a comment must contribute nothing, got {found:?}"
    );
    Ok(())
}

/// Triple quotes inside an ordinary single-line string are content, not a
/// multiline delimiter.
///
/// Counting delimiters without tracking single-line strings toggled the
/// multiline state on this text and mis-located the table.
#[rstest]
// The delimiter spelled inside a basic string.
#[case("[package]\ndescription = \"see \\\"\\\"\\\" here\"\n")]
// The same, inside a literal string, where backslashes do not escape.
#[case("[package]\ndescription = 'see \"\"\" here'\n")]
// A non-ASCII character before the table must not truncate the scan.
#[case("[package]\ndescription = \"a Ünicöde description — with dashes\"\n")]
fn string_content_does_not_toggle_the_multiline_state(#[case] preamble: &str) -> Result<()> {
    let manifest = format!("{preamble}{TABLE}locales = [\"en-US\"]\n");
    let found = metadata_locales(&manifest)
        .ok_or_else(|| anyhow!("expected the locales key to be found"))?;
    ensure!(found == ["en-US"], "got {found:?}");
    Ok(())
}
