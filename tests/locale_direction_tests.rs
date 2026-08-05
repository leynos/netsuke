//! Tests for the direction marking of right-to-left catalogues.
//!
//! Split from `locale_catalogue_tests.rs` to keep both files within the
//! repository's 400-line limit. That file checks what each catalogue declares;
//! this one checks how its rendered fragments read on a terminal.

use anyhow::{Result, ensure};
use rstest::rstest;

use netsuke::locale_catalogues::{LocaleCatalogue, SUPPORTED_LOCALES};

/// The catalogue text for `tag`.
fn catalogue_text(tag: &str) -> Result<&'static str> {
    SUPPORTED_LOCALES
        .iter()
        .find(|entry| entry.tag() == tag)
        .map(LocaleCatalogue::resource)
        .ok_or_else(|| anyhow::anyhow!("locale {tag} is not in the registry"))
}

const fn is_rtl(ch: char) -> bool {
    matches!(ch, '\u{0590}'..='\u{08FF}' | '\u{FB1D}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}')
}

/// Right-to-left marker that pins a message's paragraph direction.
const RTL_MARK: char = '\u{200F}';

/// Messages that are deliberately direction-neutral.
///
/// Each is either a bare technical token substituted into another message, or
/// an all-Latin diagnostic line. Pinning these to right-to-left would move a
/// Latin identifier to the wrong edge of the terminal, so they are exempt.
const DIRECTION_NEUTRAL: [&str; 5] = [
    // Clap's usage string, which names the binary and its Latin flags.
    "cli.usage",
    // Stream names, substituted into the command diagnostics.
    "stdlib.command.output.stream.stdout",
    "stdlib.command.output.stream.stderr",
    // An all-Latin diagnostic tag followed by its detail.
    "stdlib.which.args_error",
    // A `{symbol} {label}` composition template for accessible output.
    "semantic.prefix.rendered",
];

/// Whether `value` opens a `select` expression rather than carrying text.
///
/// The selector line renders nothing; its variants carry the text, and they
/// are checked separately.
fn opens_select(value: &str) -> bool {
    value.ends_with("->")
}

/// The text a `select` variant line renders, if the line is one.
fn variant_text(trimmed: &str) -> Option<&str> {
    trimmed
        .trim_start_matches('*')
        .strip_prefix('[')?
        .split_once(']')
        .map(|(_, rest)| rest.trim())
        .filter(|rest| !rest.is_empty())
}

/// The identifier a message line declares, with the text it renders.
///
/// The text is `None` when the line opens a `select`, because the variants
/// carry the text instead, or when the value is empty.
fn message_text(trimmed: &str) -> Option<(&str, Option<&str>)> {
    let (id, raw_value) = trimmed.split_once('=')?;
    let value = raw_value.trim();
    let rendered = (!value.is_empty() && !opens_select(value)).then_some(value);
    Some((id.trim(), rendered))
}

/// Every rendered fragment of a catalogue, as `(id, text)` pairs.
///
/// A message's own value is one fragment; each variant of a `select`
/// expression is another, because whichever variant Fluent picks becomes the
/// whole rendered string and so decides the paragraph direction on its own.
/// How a catalogue line contributes to the rendered text.
enum Fragment<'line> {
    /// Not rendered: a blank line or an entry-starting comment.
    Skipped,
    /// A `select` variant's text, belonging to the current message.
    Variant(&'line str),
    /// An indented continuation, belonging to the current message.
    Continuation(&'line str),
    /// A new message, with its own text when it has any.
    Message(&'line str, Option<&'line str>),
}

/// Classify one catalogue line.
///
/// Indentation decides before the first character does: Fluent's comment
/// syntax applies only to a line that starts an entry, so an indented line is
/// pattern text even when it begins with `#`.
fn classify(line: &str) -> Fragment<'_> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Fragment::Skipped;
    }
    let indented = line.starts_with(' ');
    if !indented && trimmed.starts_with('#') {
        return Fragment::Skipped;
    }
    if let Some(rendered) = variant_text(trimmed) {
        return Fragment::Variant(rendered);
    }
    if indented {
        return Fragment::Continuation(trimmed);
    }
    message_text(trimmed).map_or(Fragment::Skipped, |(id, rendered)| {
        Fragment::Message(id, rendered)
    })
}

/// Every rendered fragment of a catalogue, as `(id, text)` pairs.
fn rendered_fragments(text: &str) -> Vec<(String, String)> {
    let mut fragments = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        match classify(line) {
            Fragment::Skipped => {}
            Fragment::Variant(rendered) | Fragment::Continuation(rendered) => {
                fragments.push((current.clone(), rendered.to_owned()));
            }
            Fragment::Message(id, rendered) => {
                id.clone_into(&mut current);
                if let Some(body) = rendered {
                    fragments.push((current.clone(), body.to_owned()));
                }
            }
        }
    }
    fragments
}

/// A right-to-left message that opens with a Latin word, a bracket or a
/// placeable would otherwise take its paragraph direction from that token.
/// Prefixing the value with U+200F keeps the direction with the locale.
///
/// Fluent wraps every interpolated value in bidi isolates, so a template built
/// only from placeables and punctuation — `[{ $state }] { $label }` — carries
/// no strong character at all and defaults to left-to-right. Those need the
/// mark just as much as a Latin-initial sentence does, so the check covers
/// every rendered fragment rather than only those with visible script.
#[rstest]
#[case("ar")]
#[case("fa")]
#[case("he")]
fn rtl_catalogues_pin_paragraph_direction(#[case] tag: &str) -> Result<()> {
    for (id, value) in rendered_fragments(catalogue_text(tag)?) {
        if DIRECTION_NEUTRAL.contains(&id.as_str()) {
            continue;
        }
        let first = value.chars().next().unwrap_or(RTL_MARK);
        ensure!(
            first == RTL_MARK || is_rtl(first),
            "{tag}: {id} renders text starting with {first:?}, which leaves the \
             paragraph direction to that character; prefix the value with U+200F"
        );
    }
    Ok(())
}

/// An indented continuation beginning with `#` is a rendered fragment.
///
/// Classifying it as a comment dropped it from the fragment list, so
/// `rtl_catalogues_pin_paragraph_direction` would silently skip it rather than
/// check its direction marking — a gap that widens as a translator uses the
/// syntax.
#[test]
fn an_indented_hash_continuation_is_a_rendered_fragment() -> Result<()> {
    let fragments = rendered_fragments("a.key = first\n    #tagged continuation\n");
    ensure!(
        fragments
            .iter()
            .any(|(id, text)| id == "a.key" && text.contains("#tagged continuation")),
        "the indented continuation must be rendered, got {fragments:?}"
    );
    Ok(())
}

/// An unindented comment is still skipped.
#[test]
fn an_unindented_comment_is_not_a_fragment() -> Result<()> {
    let fragments = rendered_fragments("# a comment\na.key = first\n");
    ensure!(
        !fragments.iter().any(|(_, text)| text.contains("a comment")),
        "a comment must not be rendered, got {fragments:?}"
    );
    Ok(())
}
