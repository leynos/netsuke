//! Bound and escape configuration text before it appears in a diagnostic.
//!
//! A trusted definition is operator policy, so a rejected value is usually a
//! typo rather than an attack. It may still be arbitrarily long or contain
//! terminal control sequences, and a diagnostic that echoed it verbatim could
//! corrupt the operator's terminal or bury the actionable part of the message.
//! Every rejection in this module renders its input through [`bounded`].
//!
//! The discovery layer uses the same helper for the messages `serde` builds,
//! because a wrong-typed field echoes its value. That is why these helpers are
//! crate-visible rather than private to this module.

/// Limit how many characters a diagnostic quotes from a rejected value.
const MAX_DIAGNOSTIC_CHARACTERS: usize = 64;

/// Render `text` with control characters escaped and its length bounded.
///
/// Escaping keeps a diagnostic on one line: a newline or an escape sequence in
/// the rejected text appears as its `escape_debug` spelling rather than being
/// interpreted by the terminal. A trailing ellipsis states that the preview is
/// incomplete, so a reader does not mistake a prefix for the whole value.
pub(crate) fn bounded(text: &str) -> String {
    let mut escaped: String = text
        .chars()
        .take(MAX_DIAGNOSTIC_CHARACTERS)
        .flat_map(char::escape_debug)
        .collect();
    if text.chars().nth(MAX_DIAGNOSTIC_CHARACTERS).is_some() {
        escaped.push('…');
    }
    escaped
}

/// Render `text` as a bounded, double-quoted preview.
///
/// Quoting distinguishes a preview from the surrounding message while
/// [`bounded`] keeps it to one escaped line.
pub(crate) fn preview(text: &str) -> String {
    format!("\"{}\"", bounded(text))
}
