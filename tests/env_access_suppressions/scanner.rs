//! Detection of `allow` attributes that switch off a policy-carrying lint.
//!
//! The scan reads source text because that is what an attribute is: there is no
//! execution to model, and "this file does not opt out" is exactly a statement
//! about what the file says. Two properties keep it off innocent sources.
//! Comments and string or char literals are blanked by [`super::mask`] first,
//! because that is where quoted text lives and a line inside either is not a
//! line of code. An attribute is then recognized by its tokens — `#`, an
//! optional `!`, `[`, a name, `(` — so prose that quotes an attribute is not a
//! finding, since only masking decides what is prose. Finally the attribute is
//! read to its matching parenthesis, so one that `rustfmt` has wrapped across
//! several lines is read whole rather than truncated.
//!
//! An `allow` nested in a `cfg_attr` is read too. It is the same suppression
//! written one token differently, and `clippy::allow_attributes` does not fire
//! on the inner form, so nothing else reports it.
//!
//! # Why matching is token-wise and not line-wise
//!
//! The scan once anchored at the start of a line, on the reasoning that
//! `rustfmt` normalizes an attribute's spelling and `make check-fmt` enforces
//! that, so any spelling the anchor declined to read could not reach the
//! compiler. That reasoning is false, and it was falsified by measurement
//! rather than argument. `#[rustfmt::skip]` freezes the very spelling `rustfmt`
//! would otherwise normalize, and every shape below then compiles, silences the
//! policy outright (`clippy` exit 0 where the same file without the attribute
//! exits 101), and passed the line-anchored scan:
//!
//! - `#[allow` and its `(` on separate lines, under a `#[rustfmt::skip]`;
//! - a newline between `#[allow(` and the lint list;
//! - a newline between the `#` and the `[`;
//! - `r#allow(...)`, and `r#clippy::disallowed_methods`, raw identifiers;
//! - `clippy :: disallowed_methods`, with spaces around the path separator;
//! - the deprecated bare name `disallowed_methods` beside its enabler.
//!
//! Each is the same suppression one token differently, so the matcher reads
//! tokens and tolerates whitespace between them rather than trusting a layout
//! gate to have fixed the spelling first. What keeps it off prose is masking,
//! which is the mechanism that was always doing that work: a comment or a
//! string is blanked before the matcher sees it, so an attribute quoted inside
//! one is never read, whatever line it sits on. `#[expect(...)]` and
//! `.expect(...)` are excluded by the name the matcher looks for — it accepts
//! `allow` and `cfg_attr` and nothing else — rather than by their position.
//!
//! The lesson is recorded here because it is the reason the code looks the way
//! it does: a layout gate is not a proof about spelling. It normalizes what it
//! is shown, and a skip attribute is a request to be shown nothing.
//!
//! # Why `warn` is not read
//!
//! A reviewer's natural next question is why the matcher accepts `allow` and
//! `cfg_attr` but not `warn`, so the answer is measured rather than asserted. A
//! `warn` of the policy lint *does* lower it from the workspace's `deny` to
//! `warn` — bare `cargo clippy` exits 0 where the same file exits 101 — so it
//! is a real suppression and not a no-op. It is not a *silent* one: every lint
//! and test target passes `-D warnings` (`Makefile:206` among others), and that
//! re-promotes the lint to an error. Twelve spellings were probed — inner and
//! outer, `cfg_attr`-wrapped, the group and alias names, and the guard-lint
//! forms — and every one exits 101 under the gate's flags while the bare run
//! silences the same file. Reporting a shape that cannot pass a gate would be a
//! rule the code cannot justify. Whether a name belongs in the banned set is
//! decided per name by what it can reach, not by which category it looks like it
//! belongs to — the criterion [`policy`](super::policy) states and the one that
//! admits `clippy::restriction` while leaving `unknown_lints` out.
//!
//! The exception is a `warn` of the policy lint seated beside an
//! `allow(warnings)`, and it is the one measured way past the gate's flags. The
//! `warn` lowers the policy lint to `warn`, which is what puts it *into* the
//! `warnings` group — the group is the set of lints currently at `warn`, not a
//! parent of the hierarchy — and the `allow` then suppresses that group: exit 0
//! under `-D warnings`, in either order. Neither half escapes alone. The scan
//! reports this pair, because the `allow` half is what it matches and
//! `warnings` is in the banned set.

use super::mask::mask_non_code;
use super::policy::{is_offence, named_lints};

/// Which attribute form a marker opens.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AttributeKind {
    /// An `#[allow(...)]` or `#![allow(...)]` attribute.
    Allow,
    /// An `#[cfg_attr(...)]` or `#![cfg_attr(...)]` attribute.
    CfgAttr,
}

impl AttributeKind {
    /// Return the attribute form an attribute name selects.
    fn of(name: &str) -> Option<Self> {
        match name {
            "allow" => Some(Self::Allow),
            "cfg_attr" => Some(Self::CfgAttr),
            _ => None,
        }
    }
}

/// Return the offset of every attribute's `(`, with the form it opens.
///
/// The search is over the whole text rather than a line at a time, because
/// nothing about an attribute is line-shaped: its marker may be split across
/// lines, and so may the gap before its parenthesis. Masking has already
/// removed the comments and literals where quoted text lives, so a candidate
/// here is a candidate in code.
///
/// Scanning resumes just inside the parenthesis it found, so an `allow` nested
/// in a `cfg_attr` is not read twice — that inner form has no `#` of its own,
/// and [`cfg_attr_allow_bodies`] is what reads it.
fn attribute_opens(text: &str) -> Vec<(usize, AttributeKind)> {
    let mut opens = Vec::new();
    let mut index = 0_usize;
    while let Some(byte) = text.as_bytes().get(index) {
        if *byte != b'#' {
            index += 1;
            continue;
        }
        if let Some((open, kind)) = attribute_at(text, index) {
            opens.push((open, kind));
            index = open + 1;
        } else {
            index += 1;
        }
    }
    opens
}

/// Parse the attribute whose `#` sits at `hash`, returning its `(` offset.
///
/// The tokens are `#`, an optional `!`, `[`, a name, and `(`, with whitespace
/// permitted between any two of them — including a newline, which is what
/// makes this independent of where a line ends. The name is read through an
/// optional `r#` prefix, so a raw identifier names the same attribute; see
/// [`identifier`].
///
/// Returns `None` for anything else, which is what keeps `#[expect(...)]`,
/// `.expect(...)`, `#[derive(...)]`, and a bare `#` in ordinary code out of the
/// result. `#[cfg_attr(...)]` is matched too, so that its body can be searched
/// for the `allow` it may wrap.
fn attribute_at(text: &str, hash: usize) -> Option<(usize, AttributeKind)> {
    let after_hash = text.get(hash..)?.strip_prefix('#')?.trim_start();
    let after_bang = after_hash
        .strip_prefix('!')
        .unwrap_or(after_hash)
        .trim_start();
    let after_bracket = after_bang.strip_prefix('[')?.trim_start();
    let (name, after_name) = identifier(after_bracket)?;
    let kind = AttributeKind::of(name)?;
    let after_paren = after_name.trim_start().strip_prefix('(')?;
    Some((text.len() - after_paren.len() - 1, kind))
}

/// Split the Rust identifier `text` opens with, and return what follows it.
///
/// A leading `r#` is stepped over: a raw identifier denotes whatever the name
/// without the prefix denotes, so `r#allow` is the `allow` attribute and
/// `r#clippy::disallowed_methods` is the policy lint. Being blind to the prefix
/// is the whole point — it is a spelling that reaches the compiler but not a
/// reader's eye, which is what makes it worth evading with.
///
/// Returns `None` when no identifier opens the text.
fn identifier(text: &str) -> Option<(&str, &str)> {
    let body = text.strip_prefix("r#").unwrap_or(text);
    let end = body
        .find(|character| !is_identifier_char(character))
        .unwrap_or(body.len());
    (end > 0).then(|| body.split_at(end))
}

/// Whether the byte being read sits inside a double-quoted string.
#[derive(Clone, Copy)]
enum StringState {
    /// Reading code, outside every string.
    Outside,
    /// Reading a string's body, tracking whether the last byte escaped the next.
    Quoted {
        /// Whether the previous byte was an unescaped `\`.
        escaped: bool,
    },
}

impl StringState {
    /// Consume one byte, returning the next state and whether it was string text.
    ///
    /// The flag is what lets the caller skip its own handling: every byte of a
    /// string, its opening and closing quotes included, is consumed here, so a
    /// parenthesis counter downstream never sees a `)` the string quoted. A
    /// backslash escapes the byte after it and is cleared by whatever it
    /// escaped, which keeps an escaped quote from closing the string and an
    /// escaped backslash from opening an escape.
    const fn consume(self, byte: u8) -> (Self, bool) {
        match (self, byte) {
            // A backslash in a string escapes whatever follows it.
            (Self::Quoted { escaped: false }, b'\\') => (Self::Quoted { escaped: true }, true),
            // An unescaped quote is the only byte that closes the string.
            (Self::Quoted { escaped: false }, b'"') => (Self::Outside, true),
            // Every other quoted byte is content, and so is the quote that
            // opens a string from code: both leave a string being read with no
            // escape pending. An escaped byte clears the escape it was the
            // target of, and a quote behind an escape is content rather than a
            // close, which is why the two share an arm.
            (Self::Quoted { .. }, _) | (Self::Outside, b'"') => {
                (Self::Quoted { escaped: false }, true)
            }
            // Anything else is code, and the caller must handle it.
            (Self::Outside, _) => (Self::Outside, false),
        }
    }
}

/// Read the parenthesized body of the attribute whose `(` sits at `open`.
///
/// Scanning tracks parenthesis depth so an attribute that `rustfmt` has wrapped
/// across several lines is read whole. Parentheses inside a double-quoted
/// string are stepped over, and a backslash escape is honoured, so a `)` inside
/// a `reason` string does not end the scan early. Both are already unreachable
/// on [`mask_non_code`]'s output, but the tracking is what makes the function
/// correct on its own terms rather than only on that caller's.
///
/// Returns `None` when the parenthesis is never closed, which leaves the
/// malformed attribute unread rather than reporting the remainder of the file
/// as its body.
fn read_attribute_body(source: &str, open: usize) -> Option<String> {
    // `open` is the known opening parenthesis, so the body starts inside it.
    let mut depth = 1_usize;
    let mut string_state = StringState::Outside;
    for (offset, byte) in source.as_bytes().iter().enumerate().skip(open + 1) {
        let (next_state, consumed) = string_state.consume(*byte);
        string_state = next_state;
        if consumed {
            continue;
        }
        match byte {
            b'(' => depth += 1,
            b')' if depth == 1 => {
                return source.get(open + 1..offset).map(str::to_owned);
            }
            b')' => depth = depth.checked_sub(1)?,
            _ => {}
        }
    }
    None
}

/// Return the body of every `allow` nested in a `cfg_attr` body.
///
/// The nesting may be several `cfg_attr`s deep, so the whole body is searched
/// rather than its first argument. The inner form is written without a `#`, so
/// it cannot be found the way [`attribute_at`] finds the outer one; the name is
/// located instead and must be followed by a `(`. A name that merely contains
/// `allow` — such as `clippy::allow_attributes`, or the word inside a `reason`
/// string — is skipped by the test for an identifier character on either side.
fn cfg_attr_allow_bodies(body: &str) -> Vec<String> {
    let mut bodies = Vec::new();
    let mut index = 0_usize;
    while let Some(offset) = body.get(index..).and_then(|rest| rest.find("allow")) {
        let at = index + offset;
        let after = at + "allow".len();
        index = after;
        if body
            .get(..at)
            .and_then(|prefix| prefix.chars().next_back())
            .is_some_and(is_identifier_char)
        {
            continue;
        }
        let Some(tail) = body.get(after..) else {
            continue;
        };
        if tail.chars().next().is_some_and(is_identifier_char) {
            continue;
        }
        let Some(open) = tail
            .trim_start()
            .strip_prefix('(')
            .map(|rest| after + tail.len() - rest.len() - 1)
        else {
            continue;
        };
        bodies.extend(read_attribute_body(body, open));
    }
    bodies
}

/// Return the body of every `allow` attribute in `source`.
fn allow_attribute_bodies(source: &str) -> Vec<String> {
    let masked = mask_non_code(source);
    let mut bodies = Vec::new();
    for (open, kind) in attribute_opens(&masked) {
        match kind {
            AttributeKind::Allow => bodies.extend(read_attribute_body(&masked, open)),
            AttributeKind::CfgAttr => bodies.extend(
                read_attribute_body(&masked, open)
                    .map(|body| cfg_attr_allow_bodies(&body))
                    .unwrap_or_default(),
            ),
        }
    }
    bodies
}

/// Return every suppression of the policy `source` contains, as `(path, lint)`.
pub(super) fn scan_source(path: &str, source: &str) -> Vec<(String, String)> {
    let mut findings = Vec::new();
    for body in allow_attribute_bodies(source) {
        for lint in named_lints(&body) {
            if is_offence(path, &lint) {
                findings.push((path.to_owned(), lint));
            }
        }
    }
    findings
}

/// Return whether `character` continues a Rust identifier.
fn is_identifier_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

#[cfg(test)]
mod tests {
    //! Direct cases for the reading that masking hides from the scan.
    //!
    //! Masking blanks a literal's contents before the scan sees them, so the
    //! quoted-string tracking in [`read_attribute_body`] is unreachable through
    //! [`scan_source`] and no table row can pin it: measured, the masked form of
    //! `reason = "before \" after"` is `reason = "               "`, with the
    //! escaped quote and both its neighbours replaced by spaces. The function
    //! still documents and preserves standalone correctness, so its contract is
    //! asserted here, where the raw text reaches it.

    use super::read_attribute_body;
    use anyhow::{Result, ensure};

    /// The attribute body is read past a parenthesis the string merely quotes.
    #[test]
    fn a_parenthesis_inside_a_string_does_not_close_the_body() -> Result<()> {
        let source = r#"(reason = "closing ) paren", flag)"#;
        ensure!(
            read_attribute_body(source, 0).as_deref()
                == Some(r#"reason = "closing ) paren", flag"#),
            "a `)` inside a string must not close the body"
        );
        Ok(())
    }

    /// An escaped quote does not close the string that quotes it.
    ///
    /// This is the case the mask makes unreachable through the scan, and the one
    /// a reader is most likely to think is untested. Without the escape bit the
    /// `\"` would close the string early, the following `)` would close the
    /// body, and the body would be truncated — which is exactly what the
    /// mutation of that bit measures.
    #[test]
    fn an_escaped_quote_does_not_close_the_string() -> Result<()> {
        let source = r#"(reason = "before \" after ) still inside", tail)"#;
        ensure!(
            read_attribute_body(source, 0).as_deref()
                == Some(r#"reason = "before \" after ) still inside", tail"#),
            "an escaped quote must not close the string, nor let its `)` close the body"
        );
        Ok(())
    }

    /// Nested parentheses are counted rather than treated as the body's end.
    #[test]
    fn nested_parentheses_are_counted() -> Result<()> {
        let source = "(cfg_attr(all(), allow(warnings)), tail)";
        ensure!(
            read_attribute_body(source, 0).as_deref()
                == Some("cfg_attr(all(), allow(warnings)), tail"),
            "the body ends at the parenthesis matching the one that opened it"
        );
        Ok(())
    }

    /// An unterminated body is unread rather than the rest of the file.
    #[test]
    fn an_unterminated_body_is_none() -> Result<()> {
        ensure!(
            read_attribute_body("(allow(warnings)", 0).is_none(),
            "a body that never closes must not be returned"
        );
        Ok(())
    }
}
