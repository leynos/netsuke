//! Detection of `allow` attributes that switch off a policy-carrying lint.
//!
//! The scan reads source text because that is what an attribute is: there is no
//! execution to model, and "this file does not opt out" is exactly a statement
//! about what the file says. Three properties keep it off innocent sources.
//! Comments and string or char literals are blanked by [`super::mask`] first,
//! because that is where quoted text lives and a line inside either is not a
//! line of code. An attribute is then recognized only where a line begins with
//! one, so prose that quotes the attribute — the developers' guide, a mutation
//! record, a snapshot of a diagnostic — is not a finding. Finally the attribute
//! is read to its matching parenthesis, so one `rustfmt` has wrapped across
//! several lines is read whole rather than truncated.
//!
//! An `allow` nested in a `cfg_attr` is read too. It is the same suppression
//! written one token differently, and `clippy::allow_attributes` does not fire
//! on the inner form, so nothing else reports it.

use super::mask::mask_non_code;
use super::policy::{is_offence, named_lints};

/// Which attribute form begins at a line's first non-space character.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AttributeKind {
    /// An `#[allow(...)]` or `#![allow(...)]` attribute.
    Allow,
    /// An `#[cfg_attr(...)]` or `#![cfg_attr(...)]` attribute.
    CfgAttr,
}

/// Return the byte offset of the `(` and the form of the attribute `line` opens.
///
/// The anchor at the start of the line is what keeps this off prose: a
/// whole-text search also matches a citation — this module's documentation, a
/// mutation record that quotes the attribute it exists to prohibit — and
/// reports an innocent source as suppressing the policy. Matching only at the
/// start of a line also excludes `#[expect(...)]`, `.expect(...)` method calls,
/// and commented-out attributes, since [`mask_non_code`] has already blanked a
/// comment that spans lines.
///
/// Requiring the `(` on the same line is what `rustfmt` guarantees: it
/// normalizes `# ! [allow` to `#![allow` and keeps the parenthesis with the
/// path, and `make check-fmt` rejects a source that has not been through it. A
/// contributor therefore cannot evade the scan by spacing the attribute out,
/// because the spelling that would evade it does not survive the format gate.
fn attribute_open(line: &str) -> Option<(usize, AttributeKind)> {
    let trimmed = line.trim_start();
    let (rest, marker, kind) = marker(trimmed)?;
    let after_marker = rest.trim_start();
    after_marker.strip_prefix('(')?;
    let indent = line.len() - trimmed.len();
    Some((
        indent + marker.len() + (rest.len() - after_marker.len()),
        kind,
    ))
}

/// Return the text after the attribute marker that opens `trimmed`, if any.
fn marker(trimmed: &str) -> Option<(&str, &'static str, AttributeKind)> {
    [
        ("#![allow", AttributeKind::Allow),
        ("#[allow", AttributeKind::Allow),
        ("#![cfg_attr", AttributeKind::CfgAttr),
        ("#[cfg_attr", AttributeKind::CfgAttr),
    ]
    .into_iter()
    .find_map(|(prefix, kind)| Some((trimmed.strip_prefix(prefix)?, prefix, kind)))
}

/// Read the parenthesized body of the attribute whose `(` sits at `open`.
///
/// Scanning tracks parenthesis depth so an attribute `rustfmt` has wrapped
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
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in source.as_bytes().iter().enumerate().skip(open) {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'(' => depth += 1,
            b')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return source.get(open + 1..offset).map(str::to_owned);
                }
            }
            _ => {}
        }
    }
    None
}

/// Return the body of every `allow` nested in a `cfg_attr` body.
///
/// The nesting may be several `cfg_attr`s deep, so the whole body is searched
/// rather than its first argument. A name that merely contains `allow` — such
/// as `clippy::allow_attributes`, or the word inside a `reason` string — is
/// skipped by the test for an identifier character on either side of it, and by
/// the requirement that the next non-space character be `(`.
fn cfg_attr_allow_bodies(body: &str) -> Vec<String> {
    let mut bodies = Vec::new();
    for (offset, _) in body.match_indices("allow") {
        if body
            .get(..offset)
            .and_then(|prefix| prefix.chars().next_back())
            .is_some_and(is_identifier_char)
        {
            continue;
        }
        let tail = body.get(offset + "allow".len()..).unwrap_or_default();
        if tail.chars().next().is_some_and(is_identifier_char) {
            continue;
        }
        let trimmed = tail.trim_start();
        if trimmed.strip_prefix('(').is_none() {
            continue;
        }
        let open = offset + "allow".len() + (tail.len() - trimmed.len());
        bodies.extend(read_attribute_body(body, open));
    }
    bodies
}

/// Return the body of every `allow` attribute in `source`.
///
/// The line offset is tracked by summing the lengths of the lines before it, so
/// the offset handed to [`read_attribute_body`] indexes the whole source rather
/// than the line. `split_inclusive` keeps the newline, which is what makes the
/// running total exact.
fn allow_attribute_bodies(source: &str) -> Vec<String> {
    let masked = mask_non_code(source);
    let mut bodies = Vec::new();
    let mut line_start = 0_usize;
    for line in masked.split_inclusive('\n') {
        if let Some((offset, kind)) = attribute_open(line) {
            let open = line_start + offset;
            match kind {
                AttributeKind::Allow => bodies.extend(read_attribute_body(&masked, open)),
                AttributeKind::CfgAttr => bodies.extend(
                    read_attribute_body(&masked, open)
                        .map(|body| cfg_attr_allow_bodies(&body))
                        .unwrap_or_default(),
                ),
            }
        }
        line_start += line.len();
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
