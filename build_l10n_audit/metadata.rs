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
/// The header is matched only where it begins a line, so a commented-out or
/// quoted mention of the table earlier in the manifest does not capture the
/// search and return the text above the real table. The table ends at the next
/// table header, which is the next `[` at the start of a line.
fn ortho_config_table(manifest: &str) -> Option<&str> {
    const HEADER: &str = "[package.metadata.ortho_config]";
    let start = manifest
        .match_indices(HEADER)
        .find(|(start, _)| begins_a_line(manifest, *start))
        .map(|(start, _)| start)?;
    let tail = manifest.get(start.saturating_add(HEADER.len())..)?;
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

/// Whether `start` begins a line of TOML source.
///
/// Two things disqualify it. A word character earlier on the line means this is
/// the tail of a longer key — the `locales` inside `extra_locales`. Being
/// inside a multiline string means it is content rather than source: a
/// `description` written with triple quotes can contain a line reading
/// `[package.metadata.ortho_config]`, and selecting it would make the audit
/// read a prose paragraph as the table.
fn begins_a_line(table: &str, start: usize) -> bool {
    let Some(before) = table.get(..start) else {
        return false;
    };
    let line_start = before
        .rsplit('\n')
        .next()
        .is_some_and(|indent| indent.trim().is_empty());
    line_start && !inside_multiline_string(before)
}

/// The two TOML multiline string delimiters.
const MULTILINE_DELIMITERS: [&str; 2] = ["\"\"\"", "'''"];

/// Whether `prefix` ends inside a multiline string.
///
/// Each delimiter toggles the state, and the two quote styles are tracked
/// separately because neither terminates the other. Single-line strings need no
/// handling: they cannot span the newline that must precede a line-initial
/// match.
fn inside_multiline_string(prefix: &str) -> bool {
    let mut open: Option<&str> = None;
    let mut index = 0usize;
    while index < prefix.len() {
        let Some(window) = prefix.get(index..index.saturating_add(3)) else {
            break;
        };
        let delimiter = MULTILINE_DELIMITERS
            .into_iter()
            .find(|candidate| *candidate == window)
            .filter(|candidate| open.is_none_or(|current| current == *candidate));
        if let Some(found) = delimiter {
            open = if open.is_some() { None } else { Some(found) };
            index = index.saturating_add(3);
            continue;
        }
        index = index.saturating_add(1);
    }
    open.is_some()
}

/// Whether `rest` opens with `locales` followed by an `=`.
fn is_locales_assignment(rest: &str) -> bool {
    rest.strip_prefix("locales")
        .is_some_and(|after| after.trim_start().starts_with('='))
}
