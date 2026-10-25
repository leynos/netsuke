//! Shared sanitization and bounding for text shown in terminal-facing output.
//!
//! This module owns the crate's manifest-text terminal safety policy. Help
//! catalogue rendering and Ninja inline-progress labels may use its
//! sanitization helper; only progress labels should apply the scalar bound.
//! Callers must sanitize untrusted text before localization and let the output
//! backend perform its own format-specific escaping afterwards.

use std::borrow::Cow;

/// Escape control and bidi characters before displaying manifest text.
///
/// Printable Unicode remains unchanged. Newlines, carriage returns and tabs
/// become visible backslash sequences; other terminal controls use Rust's
/// visible Unicode escape notation.
///
/// # Examples
///
/// ```text
/// "first\nsecond" -> "first\\nsecond"
/// ```
#[must_use]
pub(crate) fn terminal_safe(input: &str) -> Cow<'_, str> {
    if !input.chars().any(is_terminal_control) {
        return Cow::Borrowed(input);
    }

    let mut escaped = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            control if is_terminal_control(control) => escaped.extend(control.escape_unicode()),
            printable => escaped.push(printable),
        }
    }
    Cow::Owned(escaped)
}

/// Bound text to at most `limit` Unicode scalar values, marking truncation.
///
/// The ellipsis counts toward the limit, so a truncated value retains at most
/// `limit - 1` original scalar values. A zero limit returns an empty string.
///
/// # Examples
///
/// ```text
/// bound_scalars("abcdefgh", 5) -> "abcd…"
/// ```
#[must_use]
pub(crate) fn bound_scalars(input: &str, limit: usize) -> Cow<'_, str> {
    if limit == 0 {
        return Cow::Owned(String::new());
    }

    let mut characters = input.chars();
    let prefix: String = characters.by_ref().take(limit).collect();
    if characters.next().is_none() {
        return Cow::Borrowed(input);
    }

    let mut bounded = String::with_capacity(prefix.len() + '…'.len_utf8());
    bounded.extend(prefix.chars().take(limit - 1));
    bounded.push('…');
    Cow::Owned(bounded)
}

/// Return whether a character can control terminal display or reading order.
///
/// # Examples
///
/// `\u{202e}` is a reading-order control; `é` is printable text.
pub(crate) const fn is_terminal_control(character: char) -> bool {
    matches!(
        character,
        '\0'..='\u{001F}'
            | '\u{007F}'..='\u{009F}'
            | '\u{061C}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
    )
}

#[cfg(test)]
mod tests {
    //! Pin scalar-boundary truncation and visible terminal-control escaping.

    use super::{bound_scalars, terminal_safe};
    use rstest::rstest;

    /// Preserve text that already fits the scalar limit.
    #[rstest]
    #[case("short", 80, "short")]
    #[case("é界", 2, "é界")]
    fn leaves_untruncated_text_unchanged(
        #[case] input: &str,
        #[case] limit: usize,
        #[case] expected: &str,
    ) {
        assert_eq!(bound_scalars(input, limit), expected);
    }

    /// Mark scalar-boundary truncation without splitting UTF-8 characters.
    #[rstest]
    #[case("abcdefgh", 5, "abcd…")]
    #[case("é界🦀rust", 4, "é界🦀…")]
    #[case("abcdef", 1, "…")]
    #[case("abcdef", 0, "")]
    fn truncates_at_scalar_boundaries(
        #[case] input: &str,
        #[case] limit: usize,
        #[case] expected: &str,
    ) {
        let bounded = bound_scalars(input, limit);
        assert_eq!(bounded, expected);
        assert!(bounded.chars().count() <= limit);
    }

    /// Make terminal controls and reading-order overrides visible.
    #[rstest]
    #[case("line\nnext\tend\r", "line\\nnext\\tend\\r")]
    #[case("red\u{001B}[31m", "red\\u{1b}[31m")]
    #[case("left\u{202E}right", "left\\u{202e}right")]
    fn escapes_terminal_controls(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(terminal_safe(input), expected);
    }
}
