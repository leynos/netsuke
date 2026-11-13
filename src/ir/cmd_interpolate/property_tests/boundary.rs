//! Generated `#` boundaries for the POSIX comment-rule property.
//!
//! Each boundary pairs a template prefix with the rule the shell applies to the
//! `#` that follows it, so the property can compare both POSIX traversals
//! against token recognition rather than against the production scanner.

use proptest::prelude::*;

use super::support::{POSIX_SEPARATORS, non_separator_character_strategy};

/// One generated `#` boundary together with the rule the shell applies to it.
///
/// The expectation comes from the grammar that built the prefix, not the
/// production scanner: the start of input, a separator, or an operator closer
/// ends the preceding word, while word data, an escape, a line continuation,
/// or a closing substitution delimiter leaves it open. A scanner that
/// disagrees with the interpreter fails instead of shaping the input to match
/// itself.
///
/// The property also places a second marker after `tail`, so a family whose
/// closer arrives on a later line still asserts the following text.
#[derive(Clone, Debug)]
pub(super) struct HashBoundary {
    /// Source text placed immediately before the `#`.
    pub(super) prefix: String,
    /// Lines that close a construct the prefix deliberately leaves open.
    pub(super) tail: &'static str,
    /// Whether the shell starts a comment at that `#`.
    pub(super) starts_comment: bool,
    /// Whether that comment also swallows the marker placed after `tail`.
    pub(super) tail_marker_is_commented: bool,
}

/// Generate `#` boundaries by composing a word segment with up to two others.
///
/// The first segment supplies the start of input, a separator, ordinary word
/// data, an escape, or a line continuation. Each later segment is either
/// another word segment or a balanced closer: `$(...)`, `$((...))`, a nested
/// substitution, or a group inside a substitution all leave the word open,
/// while a subshell `)` is an operator. Composing them is what a fixed family
/// list cannot express: a scanner can satisfy each rule alone and still
/// mis-handle their interaction, such as a closer followed by a continuation.
/// The last segment decides, so the oracle stays independent of the code.
///
/// Two further families cover fixed interactions a composed segment cannot
/// reach. One leaves the outer substitution open so `#` follows an inner group
/// closer directly; the property adds the closing `)` on the next line, and a
/// second marker after it, which must interpolate again. The other puts a line
/// continuation straight after a closer, so the `#` on the following line joins
/// the word instead of starting a comment.
pub(super) fn hash_boundary_strategy() -> impl Strategy<Value = HashBoundary> {
    let more = prop::collection::vec(hash_segment_strategy(), 0..=2);
    let composed =
        (word_segment_strategy(), more).prop_map(|(word, rest)| compose_boundary(word, rest));
    let inner_group = word_segment_strategy().prop_map(inner_group_closer_boundary);
    let closer_continuation = word_segment_strategy().prop_map(closer_continuation_boundary);
    prop_oneof![4 => composed, 1 => inner_group, 1 => closer_continuation]
}

/// Compose a leading word segment and up to two further segments.
fn compose_boundary(
    (mut prefix, mut starts_comment): (String, bool),
    rest: Vec<(String, bool)>,
) -> HashBoundary {
    for (segment, ends_word) in rest {
        prefix.push_str(&segment);
        starts_comment = ends_word;
    }
    HashBoundary {
        prefix,
        tail: "",
        starts_comment,
        tail_marker_is_commented: starts_comment,
    }
}

/// Place `#` directly after the closer of a group inside a substitution.
///
/// The group's `)` is an operator, so the rest of the line is a comment and the
/// marker never interpolates. The outer substitution still needs its own
/// closer, which the property places after the marker's line.
fn inner_group_closer_boundary((word, _): (String, bool)) -> HashBoundary {
    HashBoundary {
        prefix: format!("{word}$( (printf x)"),
        tail: "\n)",
        starts_comment: true,
        tail_marker_is_commented: false,
    }
}

/// Place a line continuation directly after a substitution closer.
///
/// The continuation joins the `#` to the preceding word, so the marker does not
/// start a comment and the quote after it still opens a region.
fn closer_continuation_boundary((word, _): (String, bool)) -> HashBoundary {
    HashBoundary {
        prefix: format!("{word}$(printf x)\\\n"),
        tail: "",
        starts_comment: false,
        tail_marker_is_commented: false,
    }
}

/// Generate any single segment and whether it ends the preceding word.
fn hash_segment_strategy() -> impl Strategy<Value = (String, bool)> {
    prop_oneof![3 => word_segment_strategy(), 2 => closer_segment_strategy()]
}

/// Generate a segment that supplies word text and whether it ends the word.
fn word_segment_strategy() -> impl Strategy<Value = (String, bool)> {
    let separator = prop::sample::select(POSIX_SEPARATORS.to_vec())
        .prop_map(|separator| (format!("echo x{separator}"), true));
    let start_of_input = Just(("echo ".to_owned(), true));
    let in_word = non_separator_character_strategy()
        .prop_map(|character| (format!("echo x{character}"), false));
    let escaped = prop::sample::select(vec![' ', 'a', '9', '-'])
        .prop_map(|character| (format!("echo x\\{character}"), false));
    let continuation = Just(("echo x\\\n".to_owned(), false));
    prop_oneof![separator, start_of_input, in_word, escaped, continuation]
}

/// Generate a balanced closer and whether its `)` ends the preceding word.
fn closer_segment_strategy() -> impl Strategy<Value = (String, bool)> {
    let substitution = prop::sample::select(vec![
        "$(printf x)",
        "$((1 + 2))",
        "$( (printf x) )",
        "$(printf $(printf x))",
    ])
    .prop_map(|closer| (closer.to_owned(), false));
    let subshell = Just(("(printf x)".to_owned(), true));
    prop_oneof![substitution, subshell]
}
