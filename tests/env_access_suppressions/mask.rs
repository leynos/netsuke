//! Blanking of comments and literals before the suppression scan reads text.
//!
//! The scan reads tokens rather than lines, so it cannot tell from its own
//! position whether text is code. This module is what tells it: comments and
//! string and character literals are where quoted text lives, so their contents
//! are replaced with spaces first, and something that looks like an attribute
//! is then one only when it is really in code. That is the whole mechanism
//! keeping prose — a doc comment quoting an attribute, a fixture snapshot — out
//! of the findings, and it is a stronger one than the line anchor it replaced,
//! which a `#[rustfmt::skip]` could hold open across lines.
//!
//! Blanking preserves byte offsets and newlines, so the masked text indexes
//! exactly as the source does, and only bytes belonging to a comment or literal
//! are replaced, so the result is valid UTF-8 whenever the input is. That is
//! what lets the scan run over the masked text unchanged.

/// Return `source` with every comment and string or char literal blanked.
pub(super) fn mask_non_code(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut masked = bytes.to_vec();
    let mut index = 0_usize;
    while let Some(byte) = bytes.get(index) {
        index = match byte {
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                blank_line_comment(bytes, &mut masked, index)
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                blank_block_comment(bytes, &mut masked, index)
            }
            b'\'' => blank_char_literal(source, &mut masked, index),
            b'"' | b'r' | b'b' | b'c' => blank_string_or_advance(bytes, &mut masked, index),
            _ => index + 1,
        };
    }
    String::from_utf8_lossy(&masked).into_owned()
}

/// Replace the byte at `index` with a space, leaving any newline in place.
fn blank_byte(masked: &mut [u8], index: usize) {
    if let Some(slot) = masked.get_mut(index).filter(|slot| **slot != b'\n') {
        *slot = b' ';
    }
}

/// Replace every byte of `[start, end)` with a space, leaving newlines in place.
fn blank_span(masked: &mut [u8], start: usize, end: usize) {
    for index in start..end {
        blank_byte(masked, index);
    }
}

/// Blank a `//` comment, returning the index of its newline or the input's end.
fn blank_line_comment(bytes: &[u8], masked: &mut [u8], start: usize) -> usize {
    let mut index = start;
    while let Some(byte) = bytes.get(index) {
        if *byte == b'\n' {
            return index;
        }
        blank_byte(masked, index);
        index += 1;
    }
    index
}

/// Blank a `/* */` comment, returning the index just past its terminator.
///
/// Rust block comments nest, so the terminator is the one matching the opening
/// delimiter rather than the first one seen. The opening delimiter is known and
/// blanked before the scan starts, which leaves the loop one job: decide whether
/// each byte opens a nested comment, closes the innermost one, or is content.
/// The depth therefore starts at one and cannot go below it, since the loop
/// stops the moment it reaches zero — an empty `/**/` is blanked and closed by
/// the same path as any other comment.
fn blank_block_comment(bytes: &[u8], masked: &mut [u8], start: usize) -> usize {
    blank_span(masked, start, start + 2);
    let mut depth = 1_usize;
    let mut index = start + 2;
    while depth != 0 {
        let Some(byte) = bytes.get(index) else {
            return index;
        };
        let next = bytes.get(index + 1).copied();
        index = match (*byte, next) {
            (b'/', Some(b'*')) => {
                depth += 1;
                blank_span(masked, index, index + 2);
                index + 2
            }
            (b'*', Some(b'/')) => {
                depth -= 1;
                blank_span(masked, index, index + 2);
                index + 2
            }
            _ => {
                blank_byte(masked, index);
                index + 1
            }
        };
    }
    index
}

/// Blank a char literal, or step over a quote that opens no literal.
fn blank_char_literal(source: &str, masked: &mut [u8], start: usize) -> usize {
    char_literal_end(source, start).map_or_else(
        || start + 1,
        |end| {
            blank_span(masked, start, end);
            end
        },
    )
}

/// Blank a string literal, or step over a byte that opens none.
fn blank_string_or_advance(bytes: &[u8], masked: &mut [u8], start: usize) -> usize {
    match string_quote(bytes, start) {
        Some((quote, raw)) => blank_string(bytes, masked, quote, raw),
        None => start + 1,
    }
}

/// Blank a string body, returning the index just past its terminator.
///
/// The two spellings end differently, so each is read by its own scan: a raw
/// string ends at the first `"` followed by as many `#` as opened it, while any
/// other string honours a backslash escape.
///
/// What is blanked differs between them, and the difference is not cosmetic. An
/// escaped string's contents are blanked and its closing `"` is left, because a
/// `"` cannot begin anything the scan reads. A raw string's closing `"` and its
/// hashes are blanked with its body: those hashes are a delimiter the language
/// wrote, but to a reader of the masked text they are a `#` sitting directly
/// before a `[`, which is the opening of an attribute. See
/// [`blank_raw_string`].
fn blank_string(bytes: &[u8], masked: &mut [u8], quote: usize, raw: Option<usize>) -> usize {
    match raw {
        Some(hashes) => blank_raw_string(bytes, masked, quote, hashes),
        None => blank_escaped_string(bytes, masked, quote),
    }
}

/// Blank a raw string's body and closing delimiter, returning the index past it.
///
/// A raw string's hashes are part of its delimiter rather than its contents,
/// and they are blanked with it. Leaving them would let the closing `#` of
/// `r#"abc"#` stand as the marker of an attribute when the literal is indexed —
/// `&r#"abc"#[allow(warnings)]` is `r#"abc"#` indexed by a call to a function
/// named `allow`, and it compiles and runs. Left in place, that `#` plus the
/// `[` behind it is token-for-token the opening of an `allow` attribute, so
/// harmless code was reported as a suppression. Measured on this shape before
/// the delimiter was blanked, at one false finding.
///
/// The escaped form needs no equivalent change, and the asymmetry is the
/// reason it is worth stating: a `#` there would have to sit in the body, which
/// is already blanked, and the closing `"` that remains cannot open a marker on
/// its own. A raw string is the only spelling that leaves a `#` in the text
/// after the contents are removed.
fn blank_raw_string(bytes: &[u8], masked: &mut [u8], quote: usize, hashes: usize) -> usize {
    let mut index = quote + 1;
    while let Some(byte) = bytes.get(index) {
        if *byte == b'"' && closes_raw_string(bytes, index, hashes) {
            blank_span(masked, index, index + hashes + 1);
            return index + hashes + 1;
        }
        blank_byte(masked, index);
        index += 1;
    }
    index
}

/// Blank an escaped string's body, returning the index just past its terminator.
fn blank_escaped_string(bytes: &[u8], masked: &mut [u8], quote: usize) -> usize {
    let mut escaped = false;
    let mut index = quote + 1;
    while let Some(byte) = bytes.get(index) {
        if escaped {
            escaped = false;
        } else if *byte == b'\\' {
            escaped = true;
        } else if *byte == b'"' {
            return index + 1;
        }
        blank_byte(masked, index);
        index += 1;
    }
    index
}

/// Return whether the `"` at `index` carries the `hashes` that close a raw string.
fn closes_raw_string(bytes: &[u8], index: usize, hashes: usize) -> bool {
    (1..=hashes).all(|offset| bytes.get(index + offset) == Some(&b'#'))
}

/// Return the offset of the `"` opening a string at `start`, and its raw hashes.
///
/// A prefix counts only where it stands as a token of its own, so an identifier
/// ending in `r` does not turn the string after it into a raw one. The hash
/// count is `Some` for a raw string and `None` for one that honours escapes,
/// including a byte string such as `b"..."`, which escapes like any other: a
/// raw string closes at the first `"` followed by its opening hashes, so
/// reading `b"a \" b"` as raw would end it at the escaped quote and blank
/// whatever followed it, hiding an attribute from the scan.
fn string_quote(bytes: &[u8], start: usize) -> Option<(usize, Option<usize>)> {
    let byte = *bytes.get(start)?;
    if byte == b'"' {
        return Some((start, None));
    }
    if start > 0 && bytes.get(start - 1).is_some_and(|it| is_ident_byte(*it)) {
        return None;
    }
    let (prefix, raw) = string_prefix(byte, bytes.get(start + 1).copied())?;
    let body = start + prefix;
    if raw {
        return raw_string_quote(bytes, body);
    }
    (bytes.get(body) == Some(&b'"')).then_some((body, None))
}

/// Return the prefix length of the string opening at `start` and whether it is raw.
const fn string_prefix(byte: u8, next: Option<u8>) -> Option<(usize, bool)> {
    match (byte, next) {
        // `br"..."` and `cr"..."` are raw; the `r` is the prefix's last byte.
        (b'b' | b'c', Some(b'r')) => Some((2, true)),
        // `b"..."` and `c"..."` escape like any other string.
        (b'b' | b'c', Some(b'"')) => Some((1, false)),
        // A bare `r` opens a raw string; any other byte opens none.
        (b'r', _) => Some((1, true)),
        _ => None,
    }
}

/// Return the quote offset and hash count of the raw string opening at `quote`.
///
/// A raw string carries any number of hashes, including none, so the body is
/// located by counting them rather than by assuming at least one.
fn raw_string_quote(bytes: &[u8], quote: usize) -> Option<(usize, Option<usize>)> {
    let mut index = quote;
    let mut hashes = 0_usize;
    while bytes.get(index) == Some(&b'#') {
        hashes += 1;
        index += 1;
    }
    (bytes.get(index) == Some(&b'"')).then_some((index, Some(hashes)))
}

/// Return the index just past the `'` that closes the char literal at `start`.
///
/// A quote opens a literal only where exactly one character — or one escape —
/// sits between it and a closing quote, which is what tells `'a'` from the
/// lifetime `'a`. Reading a lifetime as an unterminated literal would send the
/// scan hunting for a close and blank real code on the way, hiding whatever
/// attribute followed it.
fn char_literal_end(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.get(start) != Some(&b'\'') {
        return None;
    }
    let after = start + 1;
    let end = if bytes.get(after) == Some(&b'\\') {
        escape_end(bytes, after + 1)?
    } else {
        after + source.get(after..)?.chars().next()?.len_utf8()
    };
    (bytes.get(end) == Some(&b'\'')).then(|| end + 1)
}

/// Return the index just past the `}` closing the `\u{...}` escape at `start`.
///
/// The escape opens with `u{`; only that much anchors it, since the contents
/// are not validated here. A char literal is read to decide where it ends, not
/// to judge whether the escape is well formed.
///
/// The precision of the returned offset is not observable through the scan, and
/// this was measured rather than assumed. `char_literal_end` accepts it only
/// when a closing quote sits at exactly that index, so an offset that is too
/// small, too large, or derived from the wrong brace makes it return `None` —
/// which leaves the literal unblanked and its contents merely become text the
/// matcher finds nothing in. Four mutations of this function, including one
/// that hunts the last `}` in the whole input, all left all 47 tests passing,
/// where disabling masking outright fails three. The behaviour here is pinned
/// by inspection and by the literal-boundary cases in the spelling suite; the
/// offset itself has no test that could distinguish it.
fn unicode_escape_end(bytes: &[u8], start: usize) -> Option<usize> {
    bytes.get(start + 1).copied().filter(|byte| *byte == b'{')?;
    bytes
        .get(start + 2..)?
        .iter()
        .position(|byte| *byte == b'}')
        .map(|offset| start + offset + 3)
}

/// Return the index just past the escape sequence whose body begins at `start`.
///
/// The forms follow the language: `\u{...}` runs to its closing brace, `\x`
/// takes the two hex digits after it, and anything else escapes a single
/// character, which may occupy several bytes.
fn escape_end(bytes: &[u8], start: usize) -> Option<usize> {
    match bytes.get(start)? {
        b'u' => unicode_escape_end(bytes, start),
        b'x' => {
            let digits = bytes.get(start + 1..start + 3)?;
            digits
                .iter()
                .all(u8::is_ascii_hexdigit)
                .then_some(start + 3)
        }
        _ => Some(start + 1),
    }
}

/// Return whether `byte` continues a Rust identifier.
const fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
