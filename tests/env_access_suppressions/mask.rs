//! Blanking of comments and literals before the suppression scan reads text.
//!
//! The scan recognizes an attribute by the line it starts, so text that merely
//! quotes one must not be read as a real suppression. Comments and string and
//! character literals are where quoted text lives, so their contents are
//! replaced with spaces first; a line then begins with the attribute only when
//! the attribute is really there.
//!
//! Blanking preserves byte offsets and newlines, so the masked text indexes
//! exactly as the source does, and only bytes belonging to a comment or literal
//! are replaced, so the result is valid UTF-8 whenever the input is. That is
//! what lets the line-anchored scan run over the masked text unchanged.

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
/// delimiter rather than the first one seen.
fn blank_block_comment(bytes: &[u8], masked: &mut [u8], start: usize) -> usize {
    let mut depth = 0_usize;
    let mut index = start;
    while let Some(byte) = bytes.get(index) {
        let next = bytes.get(index + 1).copied();
        if *byte == b'/' && next == Some(b'*') {
            depth += 1;
            blank_span(masked, index, index + 2);
            index += 2;
        } else if *byte == b'*' && next == Some(b'/') {
            depth = depth.saturating_sub(1);
            blank_span(masked, index, index + 2);
            index += 2;
            if depth == 0 {
                return index;
            }
        } else {
            blank_byte(masked, index);
            index += 1;
        }
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
/// other string honours a backslash escape. The closing delimiter is left in
/// place either way, since only the contents are quoted text.
fn blank_string(bytes: &[u8], masked: &mut [u8], quote: usize, raw: Option<usize>) -> usize {
    match raw {
        Some(hashes) => blank_raw_string(bytes, masked, quote, hashes),
        None => blank_escaped_string(bytes, masked, quote),
    }
}

/// Blank a raw string's body, returning the index just past its terminator.
fn blank_raw_string(bytes: &[u8], masked: &mut [u8], quote: usize, hashes: usize) -> usize {
    let mut index = quote + 1;
    while let Some(byte) = bytes.get(index) {
        if *byte == b'"' && closes_raw_string(bytes, index, hashes) {
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

/// Return the index just past the escape sequence whose body begins at `start`.
///
/// The forms follow the language: `\u{...}` runs to its closing brace, `\x`
/// takes the two hex digits after it, and anything else escapes a single
/// character, which may occupy several bytes.
fn escape_end(bytes: &[u8], start: usize) -> Option<usize> {
    match bytes.get(start)? {
        b'u' => {
            let mut index = start + 1;
            if bytes.get(index) != Some(&b'{') {
                return None;
            }
            index += 1;
            while let Some(byte) = bytes.get(index) {
                if *byte == b'}' {
                    return Some(index + 1);
                }
                index += 1;
            }
            None
        }
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
