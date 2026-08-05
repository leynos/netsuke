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
/// A candidate must also be a header, not merely begin a line: inside a
/// multiline array a nested value such as `["decoy"],` can open a line too,
/// and taking it for a header would truncate the table above the keys that
/// follow it. Scanning continues past such lines.
fn table_end(tail: &str) -> usize {
    tail.match_indices("\n[")
        .map(|(newline, _)| newline.saturating_add(1))
        .find(|bracket| header_starts_at(tail, *bracket))
        .unwrap_or(tail.len())
}

/// Whether a table header begins at `start` within `tail`.
///
/// Three rules, each excluding one impostor. The bracket must begin a line
/// outside any string, or it is content. Its position must sit at array depth
/// zero, or it is a nested value inside a multiline array — quoted headers
/// such as `["release metadata"]` and quoted array elements are lexically
/// identical, so only the surrounding context can tell them apart. And its
/// line must read as a header, or it is malformed input no rule should match.
fn header_starts_at(tail: &str, start: usize) -> bool {
    begins_a_line(tail, start)
        && tail
            .get(..start)
            .is_some_and(|before| scan_prefix(before).array_depth == 0)
        && tail
            .get(start..)
            .and_then(|rest| rest.lines().next())
            .is_some_and(is_table_header)
}

/// Whether `line` declares a `[table]` or `[[array-of-tables]]` header.
///
/// The bracket content must read as a TOML key — dotted segments, each bare
/// or quoted — and the line must end after the closing bracket, save for a
/// comment. Array context is the caller's job: at depth zero a well-formed
/// key in brackets cannot be a value, since bare words are not TOML values
/// and a top-level line cannot open with one.
fn is_table_header(line: &str) -> bool {
    let outer = line.strip_prefix('[').unwrap_or(line);
    let body = outer.strip_prefix('[').unwrap_or(outer);
    let Some((name, rest)) = split_header_name(body) else {
        return false;
    };
    let trailing = rest.trim_start_matches(']').trim();
    header_names_a_key(name) && (trailing.is_empty() || trailing.starts_with('#'))
}

/// Split the header body at its closing bracket, honouring quoted segments.
///
/// A `]` inside a quoted segment is key content, so the split lands on the
/// first closing bracket outside quotes, with escapes honoured inside basic
/// strings. `None` means the line never closes its bracket, which no header
/// does.
fn split_header_name(body: &str) -> Option<(&str, &str)> {
    let mut index = 0;
    while let Some(ch) = body.get(index..)?.chars().next() {
        match ch {
            '"' | '\'' => {
                let inner = body.get(index.saturating_add(1)..)?;
                let close = closing_quote_at(inner, ch)?;
                index = index.saturating_add(close).saturating_add(2);
            }
            ']' => return Some((body.get(..index)?, body.get(index..)?)),
            _ => index = index.saturating_add(ch.len_utf8()),
        }
    }
    None
}

/// The byte offset of the quote closing a segment opened with `quote`.
///
/// A backslash escapes the next character in a basic (double-quoted) string,
/// so `\"` is content rather than the close; a literal (single-quoted) string
/// takes its contents verbatim. This mirrors `step_single_line`.
fn closing_quote_at(inner: &str, quote: char) -> Option<usize> {
    let mut escaped = false;
    for (index, ch) in inner.char_indices() {
        if escaped {
            escaped = false;
        } else if ch == '\\' && quote == '"' {
            escaped = true;
        } else if ch == quote {
            return Some(index);
        }
    }
    None
}

/// Whether `name` reads as a TOML key: dotted segments, bare or quoted.
///
/// Bare segments draw on the bare-key alphabet; quoted segments accept any
/// content up to their closing quote, with `\"` inside a basic string read as
/// content rather than the close.
fn header_names_a_key(name: &str) -> bool {
    let mut rest = name.trim();
    if rest.is_empty() {
        return false;
    }
    loop {
        let Some(after) = key_segment_after(rest) else {
            return false;
        };
        rest = after.trim_start();
        let Some(next) = rest.strip_prefix('.') else {
            return rest.is_empty();
        };
        rest = next.trim_start();
    }
}

/// Consume one key segment at the head of `rest`, returning what follows.
fn key_segment_after(rest: &str) -> Option<&str> {
    if let Some(quote) = rest.chars().next().filter(|ch| matches!(ch, '"' | '\'')) {
        let inner = rest.get(1..)?;
        let end = closing_quote_at(inner, quote)?;
        inner.get(end.saturating_add(1)..)
    } else {
        let end = rest
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_')))
            .unwrap_or(rest.len());
        (end > 0).then(|| rest.get(end..)).flatten()
    }
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

/// Lexical state at the end of a scanned prefix.
struct ScanState {
    /// The multiline string delimiter still open, if any.
    open: Option<&'static str>,
    /// Array brackets opened outside strings and comments and not yet closed.
    ///
    /// Table headers self-balance on their own line, so a non-zero depth
    /// means the position lies inside a multiline array value, where a
    /// line-initial bracket is a value rather than a header.
    array_depth: usize,
}

/// Whether `prefix` ends inside a multiline string.
fn inside_multiline_string(prefix: &str) -> bool {
    scan_prefix(prefix).open.is_some()
}

/// Scan `prefix`, tracking strings, comments, and array brackets.
///
/// Each multiline delimiter toggles the string state, and the two quote
/// styles are tracked separately because neither terminates the other.
/// Single-line strings are tracked so their contents cannot toggle anything;
/// comments are skipped outside strings for the same reason. Brackets are
/// counted only outside strings and comments, and the count saturates rather
/// than underflows on the `]` that closes a header's own bracket.
fn scan_prefix(prefix: &str) -> ScanState {
    let mut state = ScanState {
        open: None,
        array_depth: 0,
    };
    let mut single: Option<char> = None;
    let mut escaped = false;
    let mut in_comment = false;
    let mut chars = prefix.char_indices();
    while let Some((index, ch)) = chars.next() {
        if in_comment {
            in_comment = ch != '\n';
            continue;
        }
        if single.is_some() {
            (single, escaped) = step_single_line(single, escaped, ch);
            continue;
        }
        if let Some(found) = multiline_delimiter_at(prefix, index, state.open) {
            state.open = toggled(state.open, found);
            // Skip the delimiter's remaining two characters.
            chars.next();
            chars.next();
            continue;
        }
        if state.open.is_some() {
            continue;
        }
        (in_comment, single) = step_plain(&mut state, ch);
    }
    state
}

/// The multiline state after a delimiter is read: closed if open, else opened.
const fn toggled(open: Option<&'static str>, found: &'static str) -> Option<&'static str> {
    if open.is_some() { None } else { Some(found) }
}

/// Advance the scan by one plain character, outside strings and comments.
///
/// Returns the new comment flag and single-line string opener; the array
/// depth is adjusted in place.
const fn step_plain(state: &mut ScanState, ch: char) -> (bool, Option<char>) {
    match ch {
        '#' => (true, None),
        '[' => {
            state.array_depth = state.array_depth.saturating_add(1);
            (false, None)
        }
        ']' => {
            state.array_depth = state.array_depth.saturating_sub(1);
            (false, None)
        }
        _ => (false, single_line_opener(ch)),
    }
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
