//! Quote-aware scanning of authored shell text.
//!
//! Lint rules must not fire on a construct that appears inside a shell quote,
//! where it is ordinary text rather than syntax. The scanner tracks the three
//! states that matter — outside quotes, inside single quotes, inside double
//! quotes — and reports only the positions that are shell-active. It also
//! skips Jinja delimiters, so a `|` inside `{{ items | join(' ') }}` is a
//! template filter rather than a pipeline.

/// One shell-active occurrence of a scanned pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    /// Byte offset of the occurrence within the scanned text.
    pub start: usize,
    /// Length of the occurrence in bytes.
    pub len: usize,
}

impl Match {
    /// Build a match at `start` spanning `len` bytes.
    #[must_use]
    pub const fn new(start: usize, len: usize) -> Self {
        Self { start, len }
    }
}

/// Quote state while walking shell text.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Quote {
    /// Outside any quote.
    None,
    /// Inside a single-quoted string, where nothing is special.
    Single,
    /// Inside a double-quoted string, where a backslash still escapes.
    Double,
}

/// Which byte positions of a scanned text are shell-active.
///
/// The mask is the shared primitive: every scan below filters candidate
/// positions through it rather than re-implementing quote tracking.
#[derive(Debug, Clone)]
pub struct Mask {
    /// One flag per byte of the scanned text.
    active: Vec<bool>,
}

impl Mask {
    /// Build the mask for `text`.
    #[must_use]
    pub fn new(text: &str) -> Self {
        let mut state = Scan::new(text.len());
        let mut characters = text.char_indices().peekable();
        while let Some((index, character)) = characters.next() {
            let follower = characters.peek().map(|(_, next)| *next);
            if state.step(index, character, follower) {
                characters.next();
            }
        }
        Self {
            active: state.active,
        }
    }

    /// Report whether the byte at `offset` is shell-active.
    #[must_use]
    pub fn is_active(&self, offset: usize) -> bool {
        self.active
            .as_slice()
            .get(offset)
            .copied()
            .unwrap_or_default()
    }
}

/// Mutable state of one mask-building pass.
struct Scan {
    /// One flag per byte, filled in as the pass proceeds.
    active: Vec<bool>,
    /// The current quote state.
    quote: Quote,
    /// Whether the pass is inside a Jinja delimiter pair.
    in_jinja: bool,
}

impl Scan {
    /// Start a pass over a text of `len` bytes.
    fn new(len: usize) -> Self {
        Self {
            active: vec![false; len],
            quote: Quote::None,
            in_jinja: false,
        }
    }

    /// Consume one character, reporting whether the next one is also consumed.
    fn step(&mut self, index: usize, character: char, follower: Option<char>) -> bool {
        if self.in_jinja {
            self.in_jinja = !closes_jinja(character, follower);
            return !self.in_jinja;
        }
        if self.quote == Quote::None && opens_jinja(character, follower) {
            self.in_jinja = true;
            return true;
        }
        if self.is_escape(character) {
            return true;
        }
        self.update_quote(character);
        self.mark(index);
        false
    }

    /// Report whether `character` escapes the next one in the current state.
    const fn is_escape(&self, character: char) -> bool {
        character == '\\' && !matches!(self.quote, Quote::Single)
    }

    /// Advance the quote state for `character`.
    const fn update_quote(&mut self, character: char) {
        self.quote = match (self.quote, character) {
            (Quote::None, '\'') => Quote::Single,
            (Quote::None, '"') => Quote::Double,
            (Quote::Single, '\'') | (Quote::Double, '"') => Quote::None,
            (current, _) => current,
        };
    }

    /// Record whether the byte at `index` is shell-active.
    fn mark(&mut self, index: usize) {
        if let Some(slot) = self.active.get_mut(index) {
            *slot = self.quote == Quote::None;
        }
    }
}

/// Report whether a Jinja delimiter opens at this character pair.
const fn opens_jinja(character: char, follower: Option<char>) -> bool {
    character == '{' && matches!(follower, Some('{' | '%' | '#'))
}

/// Report whether a Jinja delimiter closes at this character pair.
const fn closes_jinja(character: char, follower: Option<char>) -> bool {
    matches!(character, '}' | '%' | '#') && matches!(follower, Some('}'))
}

/// Find every shell-active occurrence of `needle`.
#[must_use]
pub fn find_all(text: &str, needle: &str) -> Vec<Match> {
    if needle.is_empty() {
        return Vec::new();
    }
    let mask = Mask::new(text);
    text.match_indices(needle)
        .filter(|(start, _)| mask.is_active(*start))
        .map(|(start, matched)| Match::new(start, matched.len()))
        .collect()
}

/// Find every shell-active occurrence of `needle` bounded by non-word bytes.
///
/// Word boundaries keep a rule from firing on a substring: `make` must not
/// match inside `makeinfo`, and a declared path `out` must not match inside
/// `output`.
#[must_use]
pub fn find_words(text: &str, needle: &str) -> Vec<Match> {
    find_all(text, needle)
        .into_iter()
        .filter(|found| is_word_bounded(text, found.start, found.len))
        .collect()
}

/// Report whether `start..start + len` is delimited by non-word bytes.
#[must_use]
pub fn is_word_bounded(text: &str, start: usize, len: usize) -> bool {
    let bytes = text.as_bytes();
    let before = start.checked_sub(1).and_then(|index| bytes.get(index));
    let after = bytes.get(start.saturating_add(len));
    !before.is_some_and(|byte| is_word_byte(*byte))
        && !after.is_some_and(|byte| is_word_byte(*byte))
}

/// Report whether `byte` can appear inside a path or identifier word.
const fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'/')
}

/// Split `text` into shell-active command segments.
///
/// Segments are separated by `;`, `&&`, `||`, `|`, and newlines, which is
/// enough to find the leading word of each command without implementing a
/// shell grammar. Each segment is returned with its byte offset.
#[must_use]
pub fn segments(text: &str) -> Vec<(usize, &str)> {
    let mask = Mask::new(text);
    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut skip_until = 0usize;
    for (index, character) in text.char_indices() {
        if index < skip_until {
            continue;
        }
        let follower = text.get(index..).and_then(|rest| rest.chars().nth(1));
        let width = separator_width(character, follower, mask.is_active(index));
        if width == 0 {
            continue;
        }
        segments.push((start, slice(text, start, index)));
        skip_until = index.saturating_add(width);
        start = skip_until;
    }
    segments.push((start, text.get(start..).unwrap_or_default()));
    segments
}

/// Borrow `text` between two byte offsets, yielding nothing on a bad range.
fn slice(text: &str, start: usize, end: usize) -> &str {
    text.get(start..end).unwrap_or_default()
}

/// Report the width of a command separator, or zero when there is none.
const fn separator_width(character: char, follower: Option<char>, is_active: bool) -> usize {
    if !is_active {
        return 0;
    }
    match (character, follower) {
        ('&', Some('&')) | ('|', Some('|')) => 2,
        (';' | '|' | '\n', _) => 1,
        _ => 0,
    }
}

/// Report the leading word of `segment` with its offset within it.
///
/// A leading `NAME=value` assignment is skipped, because the command it
/// prefixes is the word a caller is looking for.
#[must_use]
pub fn leading_word(segment: &str) -> Option<(usize, &str)> {
    let trimmed = segment.trim_start();
    let indent = segment.len().saturating_sub(trimmed.len());
    let word = trimmed
        .split_whitespace()
        .find(|word| !word.contains('='))?;
    let offset = trimmed.find(word)?;
    Some((indent.saturating_add(offset), word))
}

#[cfg(test)]
#[path = "shellscan_tests.rs"]
mod tests;
