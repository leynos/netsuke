//! Property tests for quote-aware shell scanning.
//!
//! The example tests pin the cases a reader thinks of. These generate shell
//! text from pieces whose shell activity is known by construction — words,
//! whitespace, separators, quoted strings, escapes, Jinja delimiters, comments,
//! and multi-byte text — and hold the scanner to that expectation byte for
//! byte. The oracle is the construction, not a second copy of the scanner's
//! state machine, so a transition the scanner gets wrong cannot be copied into
//! the expectation.

use proptest::prelude::*;

use super::{Mask, find_all, find_words, leading_word, segments};

/// Generated shell text with the expected activity of every byte.
#[derive(Debug, Clone, Default)]
struct Expected {
    /// The generated text.
    text: String,
    /// Whether each byte of `text` must be shell-active.
    active: Vec<bool>,
}

impl Expected {
    /// Append text whose every character is shell syntax.
    fn push_active(&mut self, text: &str) {
        let start = self.active.len();
        self.active.resize(start.saturating_add(text.len()), false);
        for (offset, _) in text.char_indices() {
            if let Some(flag) = self.active.get_mut(start.saturating_add(offset)) {
                *flag = true;
            }
        }
        self.text.push_str(text);
    }

    /// Append text the scanner must treat as inert throughout.
    fn push_inert(&mut self, text: &str) {
        self.active
            .resize(self.active.len().saturating_add(text.len()), false);
        self.text.push_str(text);
    }

    /// Report whether the byte at `offset` must be shell-active.
    fn is_active(&self, offset: usize) -> bool {
        self.active.get(offset).copied().unwrap_or_default()
    }
}

/// One generated piece of shell text.
#[derive(Debug, Clone)]
enum Piece {
    /// A bare word, possibly multi-byte, possibly with a `#` after its start.
    Word(String),
    /// A space or tab.
    Space(&'static str),
    /// A command separator.
    Separator(&'static str),
    /// A single-quoted string; nothing inside is special.
    SingleQuoted(String),
    /// A double-quoted string whose body may carry backslash escapes.
    DoubleQuoted(String),
    /// A backslash and the character it escapes, outside any quote.
    Escape(char),
    /// A Jinja expression, statement, or comment with its delimiters.
    Jinja(&'static str, String, &'static str),
    /// A comment introduced by `#` after whitespace and ended by a newline.
    Comment(String),
}

impl Piece {
    /// Append this piece to `expected` with the activity its syntax implies.
    fn append_to(&self, expected: &mut Expected) {
        match self {
            Self::Word(text) => expected.push_active(text),
            Self::Space(text) | Self::Separator(text) => expected.push_active(text),
            Self::SingleQuoted(body) => append_quoted(expected, '\'', body),
            Self::DoubleQuoted(body) => append_quoted(expected, '"', body),
            Self::Escape(escaped) => expected.push_inert(&format!("\\{escaped}")),
            Self::Jinja(open, body, close) => {
                // Spaces keep the body's last character from pairing with the
                // closing delimiter, as `%` + `}` would.
                expected.push_inert(&format!("{open} {body} {close}"));
            }
            Self::Comment(body) => {
                // The leading space is what lets `#` open a comment; the `#`,
                // the body, and the terminating newline are all inert.
                expected.push_active(" ");
                expected.push_inert(&format!("#{body}\n"));
            }
        }
    }
}

/// Append a quoted string: the opening quote and body are inert, and the
/// closing quote, which returns the scanner to shell syntax, is active.
fn append_quoted(expected: &mut Expected, quote: char, body: &str) {
    expected.push_inert(&format!("{quote}{body}"));
    expected.push_active(&quote.to_string());
}

/// Generate one piece of shell text.
fn piece() -> impl Strategy<Value = Piece> {
    prop_oneof![
        "[a-z0-9é漢🙂_./-][a-z0-9é漢🙂_./#-]{0,6}".prop_map(Piece::Word),
        prop::sample::select(vec![" ", "\t"]).prop_map(Piece::Space),
        prop::sample::select(vec![";", "&&", "||", "|", "\n"]).prop_map(Piece::Separator),
        "[^']{0,8}".prop_map(Piece::SingleQuoted),
        double_quoted_body().prop_map(Piece::DoubleQuoted),
        any::<char>().prop_map(Piece::Escape),
        (
            prop::sample::select(vec![("{{", "}}"), ("{%", "%}"), ("{#", "#}")]),
            "[^}]{0,8}",
        )
            .prop_map(|((open, close), body)| Piece::Jinja(open, body, close)),
        "[^\n]{0,8}".prop_map(Piece::Comment),
    ]
}

/// Generate a double-quoted body of plain characters and backslash escapes.
fn double_quoted_body() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            "[^\"\\\\]",
            any::<char>().prop_map(|escaped| format!("\\{escaped}")),
        ],
        0..6,
    )
    .prop_map(|parts| parts.concat())
}

/// Generate shell text together with the activity of every byte.
fn shell_text() -> impl Strategy<Value = Expected> {
    prop::collection::vec(piece(), 0..12).prop_map(|pieces| {
        let mut expected = Expected::default();
        for generated in &pieces {
            generated.append_to(&mut expected);
        }
        expected
    })
}

/// Split `text` at the separators the oracle marks active.
///
/// This restates the documented separator set — `;`, `&&`, `||`, `|`, and a
/// newline — over the constructed activity, not over the scanner's mask.
fn expected_segments(expected: &Expected) -> Vec<(usize, &str)> {
    let text = expected.text.as_str();
    let mut found = Vec::new();
    let mut start = 0usize;
    let mut resume = 0usize;
    for (index, character) in text.char_indices() {
        if index < resume || !expected.is_active(index) {
            continue;
        }
        let rest = text.get(index..).unwrap_or_default();
        let width = if rest.starts_with("&&") || rest.starts_with("||") {
            2
        } else if matches!(character, ';' | '|' | '\n') {
            1
        } else {
            continue;
        };
        found.push((start, text.get(start..index).unwrap_or_default()));
        resume = index.saturating_add(width);
        start = resume;
    }
    found.push((start, text.get(start..).unwrap_or_default()));
    found
}

proptest::proptest! {
    /// The mask marks exactly the bytes the construction says are syntax.
    #[test]
    fn the_mask_matches_the_constructed_activity(expected in shell_text()) {
        let mask = Mask::new(&expected.text);
        for offset in 0..expected.text.len().saturating_add(2) {
            prop_assert_eq!(
                mask.is_active(offset),
                expected.is_active(offset),
                "byte {} of {:?}", offset, expected.text
            );
        }
    }

    /// `find_all` reports an occurrence exactly when it starts on syntax.
    #[test]
    fn find_all_reports_exactly_the_active_occurrences(
        expected in shell_text(),
        needle in prop::sample::select(vec![";", "&&", "|", "#", "{{", "'", "\\", "é"]),
    ) {
        let reported: Vec<usize> = find_all(&expected.text, needle)
            .into_iter()
            .map(|found| found.start)
            .collect();
        let wanted: Vec<usize> = expected
            .text
            .match_indices(needle)
            .map(|(start, _)| start)
            .filter(|start| expected.is_active(*start))
            .collect();
        prop_assert_eq!(reported, wanted, "needle {:?} in {:?}", needle, expected.text);
    }

    /// Segments split at the active separators and nowhere else.
    #[test]
    fn segments_split_at_exactly_the_active_separators(expected in shell_text()) {
        prop_assert_eq!(segments(&expected.text), expected_segments(&expected));
    }

    /// Arbitrary text never panics the scanner, and every result it reports
    /// lies on character boundaries of the text it was given.
    #[test]
    fn scanning_any_text_is_total_and_boundary_safe(
        text in any::<String>(),
        needle in "[;&|#{}'\"\\\\a-zé]{1,3}",
    ) {
        let mask = Mask::new(&text);
        for offset in 0..text.len().saturating_add(2) {
            prop_assert!(
                !mask.is_active(offset) || text.is_char_boundary(offset),
                "byte {} of {:?} is active but not a character boundary", offset, text
            );
        }
        let all = find_all(&text, &needle);
        for found in &all {
            let end = found.start.saturating_add(found.len);
            prop_assert_eq!(text.get(found.start..end), Some(needle.as_str()));
            prop_assert!(mask.is_active(found.start));
        }
        for word in find_words(&text, &needle) {
            prop_assert!(all.contains(&word), "{:?} is a word match but not a match", word);
        }
        for (offset, segment) in segments(&text) {
            let end = offset.saturating_add(segment.len());
            prop_assert_eq!(text.get(offset..end), Some(segment));
            if let Some((lead, word)) = leading_word(segment) {
                let rest = segment.get(lead..).unwrap_or_default();
                prop_assert!(rest.starts_with(word), "{:?} not at {} of {:?}", word, lead, segment);
            }
        }
    }
}
