//! Byte-level scanner for the body of a `define_keys!` invocation.
//!
//! Split out of `keys.rs` so that the extraction entry points stay readable
//! next to the scanning primitives they drive. The scanner is deliberately
//! narrow: it recognizes Rust comments and string literals well enough to step
//! over them, which is all the audit needs in order to find `=> "..."` keys
//! and the macro's closing brace.

use std::error::Error;

#[path = "byte_index.rs"]
mod byte_index;

pub(crate) use byte_index::ByteIndex;

/// A scanner over the body of a `define_keys!` invocation.
///
/// The scan needs the body two ways: as `str`, to slice out literal contents
/// without re-decoding, and as bytes, to test one character at a time. Holding
/// both on one value keeps them paired, so no caller can pass a byte slice
/// belonging to a different string from the one it slices.
pub(super) struct DefineKeysParser<'source> {
    source: &'source str,
    bytes: &'source [u8],
}

impl<'source> DefineKeysParser<'source> {
    pub(super) const fn new(source: &'source str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
        }
    }

    /// Whether `index` has run past the end of the body.
    pub(super) const fn is_exhausted(&self, index: ByteIndex) -> bool {
        index.get() >= self.bytes.len()
    }

    fn byte_at(&self, index: ByteIndex) -> Option<&u8> {
        self.bytes.get(index.get())
    }

    fn byte_is(&self, index: ByteIndex, expected: u8) -> bool {
        self.byte_at(index) == Some(&expected)
    }

    /// Parse the string literal starting at `start`, returning its value and
    /// the position just past it.
    fn parse_string_literal(
        &self,
        start: ByteIndex,
    ) -> Result<(String, ByteIndex), Box<dyn Error>> {
        if self.byte_is(start, b'"') {
            return self.parse_regular_string_literal(start);
        }
        self.parse_raw_string_literal(start)
    }

    fn parse_regular_string_literal(
        &self,
        start: ByteIndex,
    ) -> Result<(String, ByteIndex), Box<dyn Error>> {
        let content_start = start.advance(1);
        let remainder = self
            .source
            .get(content_start.get()..)
            .ok_or_else(|| "string literal start is out of range".to_owned())?;
        let mut value = String::new();
        let mut escaped = false;
        for (offset, ch) in remainder.char_indices() {
            if escaped {
                value.push(ch);
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => return Ok((value, content_start.advance(offset + 1))),
                _ => value.push(ch),
            }
        }
        Err("unterminated string literal in localization keys".into())
    }

    fn parse_raw_string_literal(
        &self,
        start: ByteIndex,
    ) -> Result<(String, ByteIndex), Box<dyn Error>> {
        let (after_prefix, has_byte_prefix) = self.parse_raw_prefix(start)?;
        if has_byte_prefix {
            return Err("byte string literals are not supported in localization keys".into());
        }
        let (hash_count, after_hashes) = self.count_hashes(after_prefix);
        if !self.byte_is(after_hashes, b'"') {
            return Err("raw string literal missing opening quote".into());
        }
        let content_start = after_hashes.advance(1);
        let end = self
            .find_raw_string_end(content_start, hash_count)
            .ok_or_else(|| "unterminated raw string literal in localization keys".to_owned())?;
        let content = end
            .retreat(hash_count + 1)
            .and_then(|content_end| self.source.get(content_start.get()..content_end.get()))
            .ok_or_else(|| "raw string slice invalid".to_owned())?;
        Ok((content.to_owned(), end))
    }

    /// Consume an optional `b` prefix and the mandatory `r`, reporting whether
    /// the literal was a byte string.
    fn parse_raw_prefix(&self, start: ByteIndex) -> Result<(ByteIndex, bool), Box<dyn Error>> {
        let has_byte_prefix = self.byte_is(start, b'b');
        let raw_marker = if has_byte_prefix {
            start.advance(1)
        } else {
            start
        };
        if !self.byte_is(raw_marker, b'r') {
            return Err("expected string literal after define_keys! =>".into());
        }
        Ok((raw_marker.advance(1), has_byte_prefix))
    }

    /// Count the run of `#` characters at `start`, returning the count and the
    /// position just past the run.
    fn count_hashes(&self, start: ByteIndex) -> (usize, ByteIndex) {
        let mut count = 0usize;
        let mut index = start;
        while self.byte_is(index, b'#') {
            count += 1;
            index = index.advance(1);
        }
        (count, index)
    }

    fn find_raw_string_end(&self, start: ByteIndex, hash_count: usize) -> Option<ByteIndex> {
        let mut index = start;
        while let Some(byte) = self.byte_at(index) {
            if *byte == b'"' && self.raw_hashes_match(index.advance(1), hash_count) {
                return Some(index.advance(hash_count + 1));
            }
            index = index.advance(1);
        }
        None
    }

    fn raw_hashes_match(&self, start: ByteIndex, count: usize) -> bool {
        (0..count).all(|offset| self.byte_is(start.advance(offset), b'#'))
    }

    fn is_line_comment(&self, index: ByteIndex) -> bool {
        self.byte_is(index, b'/') && self.byte_is(index.advance(1), b'/')
    }

    fn is_block_comment(&self, index: ByteIndex) -> bool {
        self.byte_is(index, b'/') && self.byte_is(index.advance(1), b'*')
    }

    fn skip_line_comment(&self, start: ByteIndex) -> ByteIndex {
        let mut index = start;
        while let Some(byte) = self.byte_at(index) {
            let is_newline = *byte == b'\n';
            index = index.advance(1);
            if is_newline {
                break;
            }
        }
        index
    }

    /// Skip to just past the `*/` closing the block comment opened before
    /// `start`.
    ///
    /// Rust block comments nest, so `/* /* */ */` is one comment. Stopping at
    /// the first `*/` would leave the scan inside the outer comment and read
    /// its remainder as source.
    fn skip_block_comment(&self, start: ByteIndex) -> ByteIndex {
        let mut index = start;
        let mut depth = 1usize;
        while index.advance(1).get() < self.bytes.len() {
            if self.is_block_comment(index) {
                depth = depth.saturating_add(1);
                index = index.advance(2);
                continue;
            }
            if !self.closes_block_comment(index) {
                index = index.advance(1);
                continue;
            }
            depth = depth.saturating_sub(1);
            index = index.advance(2);
            if depth == 0 {
                return index;
            }
        }
        ByteIndex::from_offset(self.bytes.len())
    }

    /// Whether a `*/` sits at `index`.
    fn closes_block_comment(&self, index: ByteIndex) -> bool {
        self.byte_is(index, b'*') && self.byte_is(index.advance(1), b'/')
    }

    fn skip_whitespace(&self, start: ByteIndex) -> ByteIndex {
        let mut index = start;
        while self.byte_at(index).is_some_and(u8::is_ascii_whitespace) {
            index = index.advance(1);
        }
        index
    }

    /// Attempts to parse a key-value pair starting at the given index.
    /// Returns the extracted key and the next index to continue parsing.
    fn try_parse_key_at_arrow(
        &self,
        index: ByteIndex,
    ) -> Result<Option<(String, ByteIndex)>, Box<dyn Error>> {
        if !self.byte_is(index, b'=') || !self.byte_is(index.advance(1), b'>') {
            return Ok(None);
        }

        let literal_start = self.skip_whitespace(index.advance(2));
        if self.is_exhausted(literal_start) {
            return Ok(None);
        }

        let (value, next) = self.parse_string_literal(literal_start)?;
        Ok(Some((value, next)))
    }

    /// Whether a string literal opens at `index`.
    ///
    /// Recognizes `"…"`, `r"…"`, and `r#*"…"#*`, plus the byte-string forms so
    /// that a `b"…"` is skipped as a literal rather than scanned as source.
    fn starts_string_literal(&self, index: ByteIndex) -> bool {
        if self.byte_is(index, b'"') {
            return true;
        }
        let raw_marker = if self.byte_is(index, b'b') {
            index.advance(1)
        } else {
            index
        };
        if !self.byte_is(raw_marker, b'r') {
            return false;
        }
        let (_, after_hashes) = self.count_hashes(raw_marker.advance(1));
        self.byte_is(after_hashes, b'"')
    }

    /// Skip the comment or string literal at `index`.
    ///
    /// Returns `None` when `index` opens neither, leaving the caller to decide
    /// what the byte means. Both the key scan and the brace scan need to step
    /// over these regions, and for the same reason: their contents are not
    /// source.
    fn skip_comment_or_literal(
        &self,
        index: ByteIndex,
    ) -> Result<Option<ByteIndex>, Box<dyn Error>> {
        if self.is_line_comment(index) {
            return Ok(Some(self.skip_line_comment(index.advance(2))));
        }
        if self.is_block_comment(index) {
            return Ok(Some(self.skip_block_comment(index.advance(2))));
        }
        if self.starts_string_literal(index) {
            let (_, next) = self.parse_string_literal(index)?;
            return Ok(Some(next));
        }
        Ok(None)
    }

    /// Whether `needle` starts at `index` and is not the tail of a longer
    /// identifier.
    ///
    /// `other_define_keys!` contains `define_keys!`, so a bare prefix test
    /// would select the wrong macro.
    fn matches_whole_identifier(&self, index: ByteIndex, needle: &str) -> bool {
        let starts_here = self
            .source
            .get(index.get()..)
            .is_some_and(|rest| rest.starts_with(needle));
        if !starts_here {
            return false;
        }
        index.retreat(1).is_none_or(|before| {
            self.byte_at(before)
                .is_none_or(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
        })
    }

    /// Offset of `needle` where it appears as source, not inside a comment or
    /// a string literal.
    ///
    /// `str::find` would match `define_keys!` written in a doc comment or
    /// quoted in a string, and the audit would then read that text as the
    /// macro body.
    pub(super) fn find_in_source(&self, needle: &str) -> Option<usize> {
        let mut index = ByteIndex::START;
        while !self.is_exhausted(index) {
            if let Ok(Some(next)) = self.skip_comment_or_literal(index) {
                index = next;
                continue;
            }
            if self.matches_whole_identifier(index, needle) {
                return Some(index.get());
            }
            index = index.advance(1);
        }
        None
    }

    /// Offset just past the `{` that opens the body, starting the search at
    /// `start`.
    ///
    /// Trivia between the macro name and its delimiter is skipped, so
    /// `define_keys! /* { */ {` opens at the real brace. Taking the commented
    /// one would make the scan treat the real brace as nested and never find
    /// the body's end.
    pub(super) fn body_start_after(&self, start: ByteIndex) -> Option<usize> {
        let mut index = start;
        while !self.is_exhausted(index) {
            if let Ok(Some(next)) = self.skip_comment_or_literal(index) {
                index = next;
                continue;
            }
            if self.byte_is(index, b'{') {
                return Some(index.advance(1).get());
            }
            index = index.advance(1);
        }
        None
    }

    /// Offset of the `}` that closes the body opening at the start of `self`.
    ///
    /// A literal that fails to parse is stepped over one byte at a time rather
    /// than reported here. The key scan runs over the same text and diagnoses
    /// malformed literals precisely; failing first, from the brace scan, would
    /// replace those messages with a vaguer one.
    ///
    /// # Errors
    ///
    /// Returns an error when the body is never closed.
    pub(super) fn find_body_end(&self) -> Result<usize, Box<dyn Error>> {
        let mut depth = 0usize;
        let mut index = ByteIndex::START;
        while !self.is_exhausted(index) {
            if let Ok(Some(next)) = self.skip_comment_or_literal(index) {
                index = next;
                continue;
            }
            if self.byte_is(index, b'}') && depth == 0 {
                return Ok(index.get());
            }
            depth = self.depth_after(index, depth);
            index = index.advance(1);
        }
        Err("define_keys! macro body is missing '}'".into())
    }

    /// The brace depth after consuming the byte at `index`.
    ///
    /// The closing brace of the body itself never reaches here; the caller
    /// returns on it while the depth is still zero.
    fn depth_after(&self, index: ByteIndex, depth: usize) -> usize {
        if self.byte_is(index, b'{') {
            depth.saturating_add(1)
        } else if self.byte_is(index, b'}') {
            depth.saturating_sub(1)
        } else {
            depth
        }
    }

    /// Consume one token at `index`, yielding any key it declares.
    ///
    /// Tokens that declare no key yield an empty string alongside the position
    /// to resume from, so the caller advances uniformly.
    pub(super) fn process_token_at(
        &self,
        index: ByteIndex,
    ) -> Result<Option<(String, ByteIndex)>, Box<dyn Error>> {
        if self.is_exhausted(index) {
            return Ok(None);
        }
        if let Some((key, next)) = self.try_parse_key_at_arrow(index)? {
            return Ok(Some((key, next)));
        }
        if let Some(next) = self.skip_comment_or_literal(index)? {
            return Ok(Some((String::new(), next)));
        }
        Ok(Some((String::new(), index.advance(1))))
    }
}
