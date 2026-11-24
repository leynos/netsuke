//! Scan state machine for the independent Bash ANSI-C model.
//!
//! Each step consumes one character and records what the shell would do with
//! it: inert comment text, an escape pair, a lowered marker, a structural
//! delimiter, or one of the quoting rules. The rules are stated here rather
//! than borrowed from the production scanner, so the property tests compare
//! two independent readings of the same text.

use super::super::super::super::{INS_TOKEN, OUTS_TOKEN};
use super::{AnsiCObservation, Region, SourceChar, SourcePosition};

/// Track the look-behind that decides whether a `$'` opens an ANSI-C string.
///
/// Only a dollar that is not the second half of a `$$` pair may open one, so
/// the state is the three-way answer the scanner needs rather than two
/// independent flags that could disagree.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lookbehind {
    /// No dollar is waiting to pair with another or to open a string.
    Idle,
    /// A single unquoted dollar awaits a possible `$$` partner.
    UnpairedDollar,
    /// A dollar awaits the apostrophe that would open an ANSI-C string.
    PendingOpener,
}

/// Track whether the next `#` sits on a POSIX word boundary.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WordBoundary {
    /// The next `#` begins a word, so it may start a comment.
    AtBoundary,
    /// The next `#` is word data, so it does not start a comment.
    InWord,
}

/// One character together with the region that governs it.
#[derive(Clone, Copy)]
struct PlainCharacter<'a> {
    /// Borrow the sequence the model is scanning.
    chars: &'a [char],
    /// Record where the character sits in that sequence.
    position: SourcePosition,
    /// Store the character being classified.
    ch: SourceChar,
    /// Store the quote region active around the character.
    region: Region,
}

/// Accumulate the mutable state one model scan needs.
pub(super) struct OracleState {
    /// Record the quote region outside every command substitution.
    region: Region,
    /// Retain each open `$( … )` region's local quote region, innermost last.
    substitutions: Vec<Region>,
    /// Record the dollar look-behind that decides an ANSI-C opener.
    lookbehind: Lookbehind,
    /// Record whether the next character is escaped by a backslash.
    is_escaped: bool,
    /// Record whether the next `#` sits on a POSIX word boundary.
    boundary: WordBoundary,
    /// Record what the scan has seen of ANSI-C strings.
    ansi_c: AnsiCObservation,
    /// Record whether an unmodelled shell construct was seen.
    pub(super) has_unmodelled_region: bool,
}

impl OracleState {
    /// Initialise the state for a prefix that begins outside every region.
    ///
    /// The model reads `$'…'` as Bash does; the POSIX alternative is refused
    /// by the lowering's own escape-bearing guard before it is reached.
    pub(super) const fn new() -> Self {
        Self {
            region: Region::Unquoted,
            substitutions: Vec::new(),
            lookbehind: Lookbehind::Idle,
            is_escaped: false,
            boundary: WordBoundary::AtBoundary,
            ansi_c: AnsiCObservation {
                is_closed: false,
                is_escaped: false,
            },
            has_unmodelled_region: false,
        }
    }

    /// Return the quote region governing the innermost open shell region.
    pub(super) fn active_region(&self) -> Region {
        self.substitutions
            .last()
            .map_or(self.region, |region| *region)
    }

    /// Report whether this boundary can take the unquoted encoding after a
    /// closed ANSI-C string.
    ///
    /// The marker must sit in unquoted text outside every substitution and not
    /// be the second character of an escape pair, because the traversal reads
    /// an escaped character as literal rather than as a substitution site.
    pub(super) fn is_closed_ansi_c_boundary(&self) -> bool {
        self.ansi_c.has_closed() && self.is_plain_unquoted()
    }

    /// Report whether an ANSI-C body took a backslash.
    pub(super) const fn has_escaped_ansi_c(&self) -> bool {
        self.ansi_c.is_escaped()
    }

    /// Report whether this boundary is unquoted, unescaped, and outside every
    /// substitution.
    pub(super) fn is_plain_unquoted(&self) -> bool {
        !self.is_escaped
            && self.active_region() == Region::Unquoted
            && self.substitutions.is_empty()
    }

    /// Change the quote region of the innermost open shell region.
    fn set_active_region(&mut self, region: Region) {
        match self.substitutions.last_mut() {
            Some(active) => *active = region,
            None => self.region = region,
        }
    }

    /// Forget any pending opener or `$$` half at a lexical boundary.
    const fn reset_lookbehind(&mut self) {
        self.lookbehind = Lookbehind::Idle;
    }

    /// Report whether this `#` begins an inert comment.
    ///
    /// A number sign is comment syntax only on a word boundary in unquoted
    /// text; elsewhere it is word data or literal quote content.
    fn starts_comment(&self, region: Region, ch: SourceChar) -> bool {
        ch.is('#') && region == Region::Unquoted && self.boundary == WordBoundary::AtBoundary
    }

    /// Report whether this parenthesis opens or closes a substitution group.
    ///
    /// Groups inside a substitution are outside the model's claim; counting
    /// the opener keeps the closer balanced so the surrounding cases are still
    /// modelled, and the prefix is skipped rather than guessed at.
    fn is_group_delimiter(&self, region: Region, ch: SourceChar) -> bool {
        let is_paren = ch.is('(') || ch.is(')');
        region == Region::Unquoted && !self.substitutions.is_empty() && is_paren
    }

    /// Consume the character at `position` and return the next position.
    ///
    /// The scan is a ladder of narrow steps so each rule stays readable on its
    /// own: inert comment text, an escape pair, a lowered marker, a structural
    /// delimiter, and finally the quoting rules for ordinary text.
    pub(super) fn consume(
        &mut self,
        chars: &[char],
        position: SourcePosition,
        ch: SourceChar,
    ) -> SourcePosition {
        let character = PlainCharacter {
            chars,
            position,
            ch,
            region: self.active_region(),
        };
        if let Some(next) = self.consume_in_comment(character) {
            return next;
        }
        if let Some(next) = self.consume_escape(character) {
            return next;
        }
        if let Some(next) = self.consume_marker(character) {
            return next;
        }
        if let Some(next) = self.consume_structure(character) {
            return next;
        }
        self.consume_plain(character);
        position.advance(1)
    }

    /// Consume one character of a comment body, if a comment is open.
    fn consume_in_comment(&mut self, character: PlainCharacter<'_>) -> Option<SourcePosition> {
        if character.region != Region::Comment {
            return None;
        }
        self.reset_lookbehind();
        if character.ch.is_newline() {
            self.set_active_region(Region::Unquoted);
            self.boundary = WordBoundary::AtBoundary;
        }
        Some(character.position.advance(1))
    }

    /// Consume the escaped character, or mark that a backslash begins one.
    fn consume_escape(&mut self, character: PlainCharacter<'_>) -> Option<SourcePosition> {
        if self.is_escaped {
            self.is_escaped = false;
            if self.active_region() == Region::AnsiC {
                self.ansi_c.mark_escaped();
            }
            if !character.ch.is_newline() {
                self.reset_lookbehind();
                self.boundary = WordBoundary::InWord;
            }
            return Some(character.position.advance(1));
        }
        if !starts_escape(character.region, character.ch) {
            return None;
        }
        self.is_escaped = true;
        Some(character.position.advance(1))
    }

    /// Consume a lowered marker, noting an unmodelled backtick region.
    fn consume_marker(&mut self, character: PlainCharacter<'_>) -> Option<SourcePosition> {
        if character.ch.is('`') && matches!(character.region, Region::Unquoted | Region::Double) {
            self.has_unmodelled_region = true;
        }
        let length = placeholder_length(character.chars, character.position)?;
        self.reset_lookbehind();
        self.boundary = WordBoundary::InWord;
        Some(character.position.advance(length))
    }

    /// Consume a substitution opener, a group delimiter, or a comment opener.
    fn consume_structure(&mut self, character: PlainCharacter<'_>) -> Option<SourcePosition> {
        if opens_command_substitution(character) {
            self.substitutions.push(Region::Unquoted);
            self.reset_lookbehind();
            self.boundary = WordBoundary::AtBoundary;
            return Some(character.position.advance(2));
        }
        if self.is_group_delimiter(character.region, character.ch) {
            self.note_group_delimiter(character.ch);
            return Some(character.position.advance(1));
        }
        if self.starts_comment(character.region, character.ch) {
            self.set_active_region(Region::Comment);
            self.reset_lookbehind();
            return Some(character.position.advance(1));
        }
        None
    }

    /// Record one parenthesis that opens or closes a group inside a substitution.
    fn note_group_delimiter(&mut self, ch: SourceChar) {
        if ch.is('(') {
            self.has_unmodelled_region = true;
            self.boundary = WordBoundary::AtBoundary;
        } else {
            self.substitutions.pop();
            self.reset_lookbehind();
            self.boundary = WordBoundary::InWord;
        }
    }

    /// Consume one character that quoting and escape rules apply to.
    fn consume_plain(&mut self, character: PlainCharacter<'_>) {
        let PlainCharacter {
            chars,
            position,
            ch,
            region,
        } = character;
        if self.opens_ansi_c(region, ch) {
            return;
        }
        if region != Region::Unquoted {
            self.closes_quote(region, ch);
            return;
        }
        self.note_unquoted(chars, position, ch);
    }

    /// Open an ANSI-C string when `ch` is the apostrophe a dollar awaits.
    ///
    /// The literal reading has no ANSI-C region at all: `$` stays a literal
    /// character and the apostrophe opens an ordinary single quote.
    fn opens_ansi_c(&mut self, region: Region, ch: SourceChar) -> bool {
        if region != Region::Unquoted || !ch.is_ansi_c_opener() {
            return false;
        }
        if self.lookbehind != Lookbehind::PendingOpener {
            return false;
        }
        self.reset_lookbehind();
        self.set_active_region(Region::AnsiC);
        true
    }

    /// End the active quote region when `ch` is its closing delimiter.
    fn closes_quote(&mut self, region: Region, ch: SourceChar) {
        self.reset_lookbehind();
        let closes = closes_region(region, ch);
        if !closes {
            return;
        }
        self.set_active_region(Region::Unquoted);
        self.ansi_c.mark_closed(region);
        self.boundary = WordBoundary::InWord;
    }

    /// Record the look-behind and boundary one unquoted character leaves.
    fn note_unquoted(&mut self, chars: &[char], position: SourcePosition, ch: SourceChar) {
        if ch.is('$') {
            self.note_dollar(chars, position);
        } else {
            self.lookbehind = Lookbehind::Idle;
        }
        self.boundary = if ch.is_quote() {
            self.open_quote(ch);
            WordBoundary::InWord
        } else {
            boundary_after(ch)
        };
    }

    /// Record the look-behind one unquoted dollar leaves behind.
    fn note_dollar(&mut self, chars: &[char], position: SourcePosition) {
        let is_second_of_pair = self.lookbehind == Lookbehind::UnpairedDollar;
        // Bash removes a backslash-newline pair before it tokenizes.
        let mut next = position.index() + 1;
        while chars.get(next) == Some(&'\\') && chars.get(next + 1) == Some(&'\n') {
            next += 2;
        }
        self.lookbehind = match is_second_of_pair {
            true => Lookbehind::Idle,
            false if chars.get(next) == Some(&'\'') => Lookbehind::PendingOpener,
            false => Lookbehind::UnpairedDollar,
        };
    }

    /// Open the ordinary quote region that `ch` begins.
    fn open_quote(&mut self, ch: SourceChar) {
        let quote = if ch.is('\'') {
            Region::Single
        } else {
            Region::Double
        };
        self.set_active_region(quote);
    }
}

/// Report whether this character opens a command-substitution region.
///
/// A `$(` inside ordinary single quotes or an ANSI-C string is literal text,
/// so only the other regions can open one.
fn opens_command_substitution(character: PlainCharacter<'_>) -> bool {
    let is_literal = matches!(character.region, Region::Single | Region::AnsiC);
    let opens =
        character.ch.is('$') && character.chars.get(character.position.index() + 1) == Some(&'(');
    opens && !is_literal
}

/// Report whether a backslash here begins an escape the model tracks.
///
/// Ordinary single-quoted text keeps a backslash literal, so it pairs with no
/// following character.
fn starts_escape(region: Region, ch: SourceChar) -> bool {
    let escapes = matches!(region, Region::Unquoted | Region::Double | Region::AnsiC);
    ch.is('\\') && escapes
}

/// Report whether `ch` closes the quote region it sits in.
///
/// Single quotes and ANSI-C strings close on an apostrophe; double quotes close
/// on a double quote.
fn closes_region(region: Region, ch: SourceChar) -> bool {
    match region {
        Region::Single | Region::AnsiC => ch.is('\''),
        Region::Double => ch.is('"'),
        Region::Unquoted | Region::Comment => false,
    }
}

/// Report the word-boundary state a plain unquoted character leaves behind.
fn boundary_after(ch: SourceChar) -> WordBoundary {
    if ch.is_word_terminator() {
        WordBoundary::AtBoundary
    } else {
        WordBoundary::InWord
    }
}

/// Return the length of a recipe marker beginning at `position`, if any.
///
/// Lowering a marker resets the look-behind, because the marker stands in for
/// source text the traversal never scanned.
fn placeholder_length(chars: &[char], position: SourcePosition) -> Option<usize> {
    let start = position.index();
    [INS_TOKEN, OUTS_TOKEN].into_iter().find_map(|token| {
        token
            .chars()
            .enumerate()
            .all(|(offset, expected)| chars.get(start + offset) == Some(&expected))
            .then_some(token.chars().count())
    })
}
