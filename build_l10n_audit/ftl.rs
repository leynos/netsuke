//! Minimal Fluent parser used by the build-time localization audit.
//!
//! Only the subset of FTL that Netsuke's catalogues use is understood: simple
//! message declarations, multi-line message bodies (including `select`
//! expressions for plural categories), comments, and variable references. Terms
//! (identifiers starting with `-`) are ignored by design because Netsuke only
//! references message identifiers from code.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;

/// Message identifiers mapped to the variables their value interpolates.
pub(super) type MessageVariables = BTreeMap<String, BTreeSet<String>>;

/// Whether `trimmed` opens a comment.
///
/// Only meaningful for a line that starts an entry: an indented line is
/// pattern text even when it begins with `#`, so callers must rule out a
/// continuation first.
fn is_comment(trimmed: &str) -> bool {
    trimmed.starts_with('#')
}

/// A continuation line belongs to the message above it.
///
/// Fluent defines U+0020 alone as indentation, so a tab-indented line is not a
/// continuation — treating it as one would attribute its variables to the
/// message above and hide a malformed entry.
fn is_continuation(line: &str, trimmed: &str) -> bool {
    !trimmed.is_empty() && line.starts_with(' ')
}

fn message_identifier(trimmed: &str) -> Option<&str> {
    let (id_raw, _) = trimmed.split_once('=')?;
    let id = id_raw.trim();
    let starts_identifier = id
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic());
    starts_identifier.then_some(id)
}

/// Collect `$variable` references from a message body.
fn collect_variables(body: &str, into: &mut BTreeSet<String>) {
    let mut rest = body;
    while let Some(offset) = rest.find('$') {
        let after = rest.get(offset + 1..).unwrap_or_default();
        let name: String = after
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect();
        if !name.is_empty() {
            into.insert(name);
        }
        rest = after;
    }
}

/// Parse a catalogue into its message identifiers and interpolated variables.
///
/// # Errors
///
/// Returns an error if the file cannot be read or declares no messages.
pub(super) fn parse_catalogue(source: &str) -> Result<MessageVariables, Box<dyn Error>> {
    let mut messages = MessageVariables::new();
    let mut current: Option<String> = None;

    for line in source.lines() {
        let trimmed = line.trim();
        // A blank line does not end a pattern in Fluent; only the next entry
        // does. Clearing here would drop the variables of every continuation
        // after it.
        if trimmed.is_empty() {
            continue;
        }
        // Continuation is tested before comment. Fluent's comment syntax
        // applies only to a line that starts an entry, so an indented line is
        // pattern text whatever its first character — including `#`. Testing
        // for a comment first would discard such a line and lose the
        // variables it references.
        if is_continuation(line, trimmed) {
            append_continuation(&mut messages, current.as_deref(), trimmed);
            continue;
        }
        if is_comment(trimmed) {
            continue;
        }
        current = start_message(&mut messages, trimmed);
    }

    if messages.is_empty() {
        return Err("no Fluent messages found in the catalogue".into());
    }
    Ok(messages)
}

fn append_continuation(messages: &mut MessageVariables, current: Option<&str>, trimmed: &str) {
    let Some(variables) = current.and_then(|id| messages.get_mut(id)) else {
        return;
    };
    collect_variables(trimmed, variables);
}

/// Begin a new message, returning its identifier when the line declares one.
fn start_message(messages: &mut MessageVariables, trimmed: &str) -> Option<String> {
    if trimmed.is_empty() {
        return None;
    }
    let id = message_identifier(trimmed)?;
    let value = trimmed.split_once('=').map(|(_, value)| value)?;
    let variables = messages.entry(id.to_owned()).or_default();
    collect_variables(value, variables);
    Some(id.to_owned())
}
