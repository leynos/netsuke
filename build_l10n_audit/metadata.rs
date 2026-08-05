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
/// table header — the next `[` beginning a line — judged by the same rule as
/// the header itself, so a `[` inside a multiline string is content rather
/// than a boundary and cannot truncate the table early.
fn ortho_config_table(manifest: &str) -> Option<&str> {
    const HEADER: &str = "[package.metadata.ortho_config]";
    let start = manifest
        .match_indices(HEADER)
        .find(|(start, _)| begins_a_line(manifest, *start))
        .map(|(start, _)| start)?;
    let tail = manifest.get(start.saturating_add(HEADER.len())..)?;
    tail.get(..table_end(tail))
}

/// Where the table body ends within `tail`.
///
/// This is the offset of the next table header, or the whole length when no
/// further header follows. Scanning `tail` alone is sound because the header
/// match above already established that the header does not sit inside a
/// multiline string, so the string state at the start of `tail` is "outside".
///
/// A candidate must also read as a header, not merely begin a line: inside a
/// multiline array a nested value such as `["decoy"],` can open a line too,
/// and taking it for a header would truncate the table above the keys that
/// follow it. Scanning continues past such lines.
fn table_end(tail: &str) -> usize {
    tail.match_indices("\n[")
        .map(|(newline, _)| newline.saturating_add(1))
        .find(|bracket| {
            begins_a_line(tail, *bracket)
                && tail
                    .get(*bracket..)
                    .and_then(|rest| rest.lines().next())
                    .is_some_and(is_table_header)
        })
        .unwrap_or(tail.len())
}

/// Whether `line` declares a `[table]` or `[[array-of-tables]]` header.
///
/// A line-initial bracket is ambiguous in TOML: it may open a header, or a
/// nested array value inside a multiline array. The two are told apart by
/// content and tail. A header names a key from the bare-key alphabet and ends
/// its line after the closing bracket, save for a comment; an array value
/// carries quotes, commas, or a trailing comma, none of which a header line
/// may. A lone element like `[123]` that is also a well-formed bare-key header
/// stays ambiguous and is read as a header, which this manifest's metadata —
/// string-valued throughout — cannot produce as a value.
fn is_table_header(line: &str) -> bool {
    let outer = line.strip_prefix('[').unwrap_or(line);
    let body = outer.strip_prefix('[').unwrap_or(outer);
    let Some((name, rest)) = body.split_once(']') else {
        return false;
    };
    let named_bare = !name.trim().is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ' ' | '\t'));
    let trailing = rest.trim_start_matches(']').trim();
    named_bare && (trailing.is_empty() || trailing.starts_with('#'))
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
    let mut single: Option<char> = None;
    let mut escaped = false;
    let mut chars = prefix.char_indices();
    while let Some((index, ch)) = chars.next() {
        if single.is_some() {
            (single, escaped) = step_single_line(single, escaped, ch);
            continue;
        }
        let Some(found) = multiline_delimiter_at(prefix, index, open) else {
            single = open.is_none().then(|| single_line_opener(ch)).flatten();
            continue;
        };
        open = if open.is_some() { None } else { Some(found) };
        // Skip the delimiter's remaining two characters.
        chars.next();
        chars.next();
    }
    open.is_some()
}

/// Advance a single-line string scan by one character.
///
/// Returns the quote still open, if any, and whether the next character is
/// escaped. Only a basic string honours the escape; a literal string takes its
/// contents verbatim.
const fn step_single_line(single: Option<char>, escaped: bool, ch: char) -> (Option<char>, bool) {
    let Some(quote) = single else {
        return (None, false);
    };
    if escaped {
        return (Some(quote), false);
    }
    if ch == '\\' && quote == '"' {
        return (Some(quote), true);
    }
    if ch == quote || ch == '\n' {
        return (None, false);
    }
    (Some(quote), false)
}

/// The multiline delimiter starting at `index`, if one does.
///
/// A window that does not land on a character boundary is not a delimiter, so
/// `get` returning `None` yields `None` rather than ending the scan — a
/// multi-byte character earlier in the manifest must not truncate it.
fn multiline_delimiter_at(prefix: &str, index: usize, open: Option<&str>) -> Option<&'static str> {
    let window = prefix.get(index..index.saturating_add(3))?;
    MULTILINE_DELIMITERS
        .into_iter()
        .find(|candidate| *candidate == window)
        .filter(|candidate| open.is_none_or(|current| current == *candidate))
}

/// The quote opening a single-line string, if `ch` is one.
const fn single_line_opener(ch: char) -> Option<char> {
    match ch {
        '"' | '\'' => Some(ch),
        _ => None,
    }
}

/// Whether `rest` opens with `locales` followed by an `=`.
fn is_locales_assignment(rest: &str) -> bool {
    rest.strip_prefix("locales")
        .is_some_and(|after| after.trim_start().starts_with('='))
}
