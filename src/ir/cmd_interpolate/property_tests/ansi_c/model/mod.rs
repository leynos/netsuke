//! Independent model of Bash ANSI-C opening for the ANSI-C property tests.
//!
//! The model is written from first principles so that a production scanner
//! which drifts from the interpreter is reported by these tests rather than
//! described by them. It records quote regions, backslash escapes,
//! backslash-newline continuations, `$$` run parity, and the resets at
//! comments, `$( … )` boundaries, and lowered markers. Heredocs, backtick
//! bodies, and parenthesized groups inside a substitution are outside its
//! claim, and a prefix containing one is skipped instead of guessed at.
//!
//! This module owns the summary the properties read. The scan state machine
//! that produces it lives in the `state` child.

mod state;

use state::OracleState;

/// Name the shell region that governs interpolation at one source position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Region {
    /// Executable text outside every quote.
    Unquoted,
    /// Ordinary single quotes, where a backslash is literal.
    Single,
    /// Double quotes, where a backslash still escapes.
    Double,
    /// Bash ANSI-C quotes, where a backslash escapes and only `'` closes.
    AnsiC,
    /// Comment text, inert until the newline that ends it.
    Comment,
}

/// One position in the character sequence the model scans.
///
/// Wrapping the index keeps call sites honest about what a bare number means
/// and gives the position arithmetic one place to live.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct SourcePosition(usize);

impl SourcePosition {
    /// Wrap a raw index as a scan position.
    pub(super) const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Return this position as a slice index.
    pub(super) const fn index(self) -> usize {
        self.0
    }

    /// Return the position `count` characters later.
    const fn advance(self, count: usize) -> Self {
        Self(self.0 + count)
    }

    /// Return the character at this position, if one remains.
    fn character(self, chars: &[char]) -> Option<char> {
        chars.get(self.0).copied()
    }
}

/// One character of shell source text.
///
/// Naming the character keeps the scan rules about source text rather than
/// about a bare `char`, and gives the few character predicates one home.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SourceChar(char);

impl SourceChar {
    /// Wrap one source character for the scan rules.
    pub(super) const fn new(ch: char) -> Self {
        Self(ch)
    }

    /// Report whether this character is `expected`.
    const fn is(self, expected: char) -> bool {
        self.0 == expected
    }

    /// Report whether this character ends a line.
    const fn is_newline(self) -> bool {
        self.is('\n')
    }

    /// Report whether this character begins an ordinary quote region.
    const fn is_quote(self) -> bool {
        self.is('\'') || self.is('"')
    }

    /// Report whether this character is the apostrophe that opens ANSI-C.
    const fn is_ansi_c_opener(self) -> bool {
        self.is('\'')
    }

    /// Report whether this character separates POSIX words.
    const fn is_word_terminator(self) -> bool {
        matches!(
            self.0,
            ' ' | '\t' | '\n' | ';' | '&' | '|' | '(' | ')' | '<' | '>'
        )
    }
}

/// Record what one scan has seen of ANSI-C strings.
#[derive(Default)]
pub(super) struct AnsiCObservation {
    /// Record whether a string has been opened and closed again.
    is_closed: bool,
    /// Record whether a string body took a backslash.
    is_escaped: bool,
}

impl AnsiCObservation {
    /// Record that a string closed in `region`.
    pub(super) fn mark_closed(&mut self, region: Region) {
        self.is_closed |= region == Region::AnsiC;
    }

    /// Record that a string body took a backslash.
    pub(super) fn mark_escaped(&mut self) {
        self.is_escaped = true;
    }

    /// Report whether a string has been opened and closed again.
    pub(super) const fn has_closed(&self) -> bool {
        self.is_closed
    }

    /// Report whether a string body took a backslash.
    pub(super) const fn is_escaped(&self) -> bool {
        self.is_escaped
    }
}

/// Summarize the ANSI-C regions of one generated prefix.
#[derive(Default)]
pub(super) struct AnsiCModel {
    /// Record the first boundary inside an open ANSI-C string.
    pub(super) first_ansi_c: Option<SourcePosition>,
    /// Record the last boundary both `$'` readings encode as unquoted text
    /// after a string closed under the ANSI-C reading.
    pub(super) last_unquoted_after_closed: Option<SourcePosition>,
    /// Record whether a construct this model does not claim was seen.
    pub(super) has_unmodelled_region: bool,
    /// Record whether an ANSI-C string's body contained a backslash.
    ///
    /// Lowering refuses that shape outright, because its extent differs
    /// between the ANSI-C and POSIX readings, so the properties that expect a
    /// lowered result skip those prefixes.
    pub(super) has_escaped_ansi_c: bool,
}

impl AnsiCModel {
    /// Scan `chars` from first principles, without the production scanner.
    ///
    /// The text is read twice, once honouring `$'…'` as Bash does and once
    /// reading `$` as literal, because a `Posix` recipe may run under either
    /// interpreter. A marker is only expected to take the unquoted encoding
    /// where both readings put it outside every quote; where they disagree the
    /// recipe is rejected instead, and no property asserts an encoding there.
    pub(super) fn scan(chars: &[char]) -> Self {
        let reading = Reading::scan(chars);
        Self {
            first_ansi_c: reading.first_ansi_c,
            last_unquoted_after_closed: reading.closed_and_unquoted.last().copied(),
            has_unmodelled_region: reading.has_unmodelled_region,
            has_escaped_ansi_c: reading.has_escaped_ansi_c,
        }
    }
}

/// Summarize what one reading of `$'` saw across a generated prefix.
#[derive(Default)]
struct Reading {
    /// Record the first boundary inside an open ANSI-C string.
    first_ansi_c: Option<SourcePosition>,
    /// Retain every boundary that is unquoted, unescaped, and outside every
    /// substitution after an ANSI-C string has closed.
    closed_and_unquoted: Vec<SourcePosition>,
    /// Retain every boundary that is plain unquoted text.
    plain_unquoted: Vec<SourcePosition>,
    /// Record whether a construct this model does not claim was seen.
    has_unmodelled_region: bool,
    /// Record whether an ANSI-C body contained a backslash.
    has_escaped_ansi_c: bool,
}

impl Reading {
    /// Scan `chars` under one reading of `$'`.
    fn scan(chars: &[char]) -> Self {
        let mut state = OracleState::new();
        let mut summary = Self::default();
        let mut position = SourcePosition::new(0);
        summary.observe(&state, position);
        while let Some(ch) = position.character(chars) {
            position = state.consume(chars, position, SourceChar::new(ch));
            summary.observe(&state, position);
        }
        summary.has_unmodelled_region = state.has_unmodelled_region;
        summary.has_escaped_ansi_c = state.has_escaped_ansi_c();
        summary
    }

    /// Record what the state at one boundary means for the properties.
    ///
    /// Positions are pushed in scan order, so each list stays sorted and the
    /// two readings can be intersected by binary search.
    fn observe(&mut self, state: &OracleState, position: SourcePosition) {
        self.has_escaped_ansi_c |= state.has_escaped_ansi_c();
        if state.active_region() == Region::AnsiC {
            self.first_ansi_c.get_or_insert(position);
        }
        if !state.is_plain_unquoted() {
            return;
        }
        self.plain_unquoted.push(position);
        if state.is_closed_ansi_c_boundary() {
            self.closed_and_unquoted.push(position);
        }
    }
}
