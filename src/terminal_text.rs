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
    use proptest::prelude::*;
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

    proptest! {
        #[test]
        fn bounds_arbitrary_unicode_by_scalar_count(
            scalars in prop::collection::vec(any::<char>(), 0..=160),
            limit in 0usize..=100,
        ) {
            let input: String = scalars.into_iter().collect();
            let bounded = bound_scalars(&input, limit);
            let bounded_len = bounded.chars().count();

            prop_assert!(bounded_len <= limit);

            let input_len = input.chars().count();
            if limit == 0 {
                prop_assert!(bounded.is_empty());
            } else if input_len <= limit {
                prop_assert_eq!(bounded.as_ref(), input.as_str());
            } else {
                let prefix: String = input.chars().take(limit - 1).collect();
                prop_assert_eq!(bounded_len, limit);
                prop_assert!(bounded.ends_with('…'));
                prop_assert_eq!(bounded.strip_suffix('…'), Some(prefix.as_str()));
            }
        }

        #[test]
        fn makes_arbitrary_and_injected_terminal_controls_visible(
            arbitrary_scalars in prop::collection::vec(any::<char>(), 0..=160),
            injected_controls in prop::collection::vec(
                prop_oneof![
                    (0_u8..=0x1f).prop_map(char::from),
                    (0x7f_u8..=0x9f).prop_map(char::from),
                    prop::sample::select(vec![
                        '\u{061C}', '\u{200E}', '\u{200F}', '\u{202A}',
                        '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}',
                        '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
                    ]),
                ],
                1..=24,
            ),
        ) {
            let arbitrary: String = arbitrary_scalars.iter().copied().collect();
            let mut input = arbitrary.clone();
            input.extend(injected_controls.iter().copied());
            let is_unsafe = |character: char| matches!(
                character,
                '\u{0000}'..='\u{001F}'
                    | '\u{007F}'..='\u{009F}'
                    | '\u{061C}' | '\u{200E}' | '\u{200F}'
                    | '\u{202A}'..='\u{202E}'
                    | '\u{2066}'..='\u{2069}'
            );

            let safe = terminal_safe(&input);
            prop_assert!(safe.chars().all(|character| !is_unsafe(character)));

            let printable: String = arbitrary_scalars
                .iter()
                .copied()
                .filter(|character| !is_unsafe(*character))
                .collect();
            let safe_printable = terminal_safe(&printable);
            prop_assert_eq!(safe_printable.as_ref(), printable.as_str());

            let mut visible_suffix = String::new();
            for control in injected_controls {
                let visible = match control {
                    '\n' => "\\n".to_owned(),
                    '\r' => "\\r".to_owned(),
                    '\t' => "\\t".to_owned(),
                    other => format!("\\u{{{:x}}}", u32::from(other)),
                };
                visible_suffix.push_str(&visible);
            }
            prop_assert!(safe.ends_with(&visible_suffix));
        }
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
