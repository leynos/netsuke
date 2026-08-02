//! Reading the `ortho_config` locale list out of `Cargo.toml`.
//!
//! Cargo metadata cannot call into Rust, so the locale list is necessarily
//! duplicated between the registry and the manifest. The audit compares the
//! two, which means it has to read the manifest without pulling in a TOML
//! parser as a build dependency. This module does that reading, narrowly.

/// Read the `locales = [...]` array from the `ortho_config` metadata table.
///
/// The key is matched as a whole assignment at the start of a line, so neither
/// a comment mentioning locales nor a neighbouring key such as `extra_locales`
/// can be picked up in its place. The array itself may span lines.
///
/// Returns `None` when the table or the key is absent, or when the array is
/// unterminated.
pub(super) fn parse_metadata_locales(manifest: &str) -> Option<Vec<&str>> {
    let table = ortho_config_table(manifest)?;
    let assignment = locales_assignment(table)?;
    let (_, open) = assignment.split_once('[')?;
    let (entries, _) = open.split_once(']')?;
    Some(
        entries
            .split(',')
            .map(|entry| entry.trim().trim_matches('"'))
            .filter(|entry| !entry.is_empty())
            .collect(),
    )
}

/// The body of the `[package.metadata.ortho_config]` table.
///
/// The table ends at the next table header, which is the next `[` at the start
/// of a line.
fn ortho_config_table(manifest: &str) -> Option<&str> {
    let tail = manifest.split("[package.metadata.ortho_config]").nth(1)?;
    Some(tail.split("\n[").next().unwrap_or(tail))
}

/// The table text from the `locales` assignment onwards.
fn locales_assignment(table: &str) -> Option<&str> {
    table
        .match_indices("locales")
        .filter(|(start, _)| begins_a_line(table, *start))
        .find_map(|(start, _)| {
            let rest = table.get(start..)?;
            is_locales_assignment(rest).then_some(rest)
        })
}

/// Whether `start` is preceded only by whitespace on its line.
///
/// This is what distinguishes the `locales` key from the `locales` inside
/// `extra_locales`, which has a word character before it.
fn begins_a_line(table: &str, start: usize) -> bool {
    table.get(..start).is_some_and(|before| {
        before
            .rsplit('\n')
            .next()
            .is_some_and(|indent| indent.trim().is_empty())
    })
}

/// Whether `rest` opens with `locales` followed by an `=`.
fn is_locales_assignment(rest: &str) -> bool {
    rest.strip_prefix("locales")
        .is_some_and(|after| after.trim_start().starts_with('='))
}
