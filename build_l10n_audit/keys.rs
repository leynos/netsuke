//! Extraction of Fluent message identifiers declared in Rust source.
//!
//! Parses the `define_keys!` macro in `src/localization/keys.rs` so the build
//! audit can compare the keys the code references against the keys each
//! catalogue provides.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::Path;

const DEFINE_KEYS_MACRO: &str = "define_keys!";

/// Extracts localization key values from `keys.rs`.
///
/// Parses the `define_keys!` macro invocation to extract Fluent key identifiers.
/// Expects entries of the form: `CONST_NAME => "fluent-key-id",` within the
/// macro body.
///
/// Implementation note: uses `extract_define_keys_body` to locate the macro
/// body and `parse_define_keys_body` to read values from `=> "..."` patterns.
///
/// # Errors
///
/// Returns an error if the macro cannot be parsed or no keys are found.
pub(super) fn extract_key_constants(path: &Path) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let source = fs::read_to_string(path)?;
    let body = extract_define_keys_body(&source)?;
    let keys = parse_define_keys_body(body)?;
    if keys.is_empty() {
        return Err(format!("no localization keys found in {}", path.display()).into());
    }
    Ok(keys)
}

fn extract_define_keys_body(source: &str) -> Result<&str, Box<dyn Error>> {
    let Some(macro_pos) = source.find(DEFINE_KEYS_MACRO) else {
        return Err("define_keys! macro not found in localization keys".into());
    };
    let after_macro = source
        .get(macro_pos + DEFINE_KEYS_MACRO.len()..)
        .ok_or_else(|| "define_keys! macro start is out of range".to_owned())?;
    let Some(open_brace) = after_macro.find('{') else {
        return Err("define_keys! macro body is missing '{'".into());
    };
    let body_start = macro_pos + DEFINE_KEYS_MACRO.len() + open_brace + 1;
    let remainder = source
        .get(body_start..)
        .ok_or_else(|| "define_keys! macro body is out of range".to_owned())?;
    let body_len = find_matching_brace(remainder)?;
    let body_end = body_start + body_len;
    source
        .get(body_start..body_end)
        .ok_or_else(|| "define_keys! macro body slice invalid".into())
}

fn find_matching_brace(source: &str) -> Result<usize, Box<dyn Error>> {
    let mut depth = 0usize;
    for (offset, ch) in source.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                if depth == 0 {
                    return Ok(offset);
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    Err("define_keys! macro body is missing '}'".into())
}

/// A byte offset into a [`DefineKeysParser`]'s source.
///
/// Positions and counts are both `usize` underneath, and the scanner passes
/// them side by side — a raw string literal's opening index next to its run of
/// hashes. Naming the position separately keeps the two from being swapped.
#[derive(Clone, Copy)]
struct ByteIndex(usize);

impl ByteIndex {
    /// The start of the parsed body.
    const START: Self = Self(0);

    const fn get(self) -> usize {
        self.0
    }

    /// The position `delta` bytes further along.
    const fn advance(self, delta: usize) -> Self {
        Self(self.0 + delta)
    }

    /// The position `delta` bytes earlier, or `None` when that would underflow.
    const fn retreat(self, delta: usize) -> Option<Self> {
        match self.0.checked_sub(delta) {
            Some(offset) => Some(Self(offset)),
            None => None,
        }
    }
}

/// A scanner over the body of a `define_keys!` invocation.
///
/// The scan needs the body two ways: as `str`, to slice out literal contents
/// without re-decoding, and as bytes, to test one character at a time. Holding
/// both on one value keeps them paired, so no caller can pass a byte slice
/// belonging to a different string from the one it slices.
struct DefineKeysParser<'source> {
    source: &'source str,
    bytes: &'source [u8],
}

impl<'source> DefineKeysParser<'source> {
    const fn new(source: &'source str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
        }
    }

    /// Whether `index` has run past the end of the body.
    const fn is_exhausted(&self, index: ByteIndex) -> bool {
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

    fn skip_block_comment(&self, start: ByteIndex) -> ByteIndex {
        let mut index = start;
        while index.advance(1).get() < self.bytes.len() {
            if self.byte_is(index, b'*') && self.byte_is(index.advance(1), b'/') {
                return index.advance(2);
            }
            index = index.advance(1);
        }
        ByteIndex(self.bytes.len())
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

    /// Consume one token at `index`, yielding any key it declares.
    ///
    /// Tokens that declare no key yield an empty string alongside the position
    /// to resume from, so the caller advances uniformly.
    fn process_token_at(
        &self,
        index: ByteIndex,
    ) -> Result<Option<(String, ByteIndex)>, Box<dyn Error>> {
        if self.is_exhausted(index) {
            return Ok(None);
        }
        if self.is_line_comment(index) {
            return Ok(Some((
                String::new(),
                self.skip_line_comment(index.advance(2)),
            )));
        }
        if self.is_block_comment(index) {
            return Ok(Some((
                String::new(),
                self.skip_block_comment(index.advance(2)),
            )));
        }
        if let Some((key, next)) = self.try_parse_key_at_arrow(index)? {
            return Ok(Some((key, next)));
        }
        Ok(Some((String::new(), index.advance(1))))
    }
}

fn parse_define_keys_body(body: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let parser = DefineKeysParser::new(body);
    let mut keys = BTreeSet::new();
    let mut index = ByteIndex::START;
    while !parser.is_exhausted(index) {
        let Some((value, next)) = parser.process_token_at(index)? else {
            break;
        };
        if !value.is_empty() {
            keys.insert(value);
        }
        index = next;
    }
    Ok(keys)
}
