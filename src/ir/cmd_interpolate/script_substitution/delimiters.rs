//! Recognise and track `$()` delimiters inside one POSIX or Bash script.
//!
//! The `)` that closes a command substitution stays part of the surrounding
//! word, so its position must survive into the next comment check. These
//! helpers own that nesting state, keeping the parent traversal within the
//! repository's module-size cap.

use super::super::case_nesting::{CaseNesting, CaseObservation};
use super::super::command_substitution::{
    SourceText, SubstitutionBoundary, reject_ambiguous_case_comment, reject_unplaced_case,
};
use super::super::posix_lexical::SubstitutionKind;
use super::super::{RejectionReason, invalid_command_error};
use super::{IrGenError, ScriptSubstitutionTraversal, ShellQuoteContext};

/// Track one nested POSIX command substitution while traversing a script.
pub(super) struct ScriptCommandSubstitution {
    /// Record the quote context local to this command substitution.
    ///
    /// The parent traversal reads and updates this field directly, so it is
    /// visible to the enclosing module rather than only to this one.
    pub(super) quote_context: ShellQuoteContext,
    /// Count nested parenthesised groups until the closing delimiter.
    parenthesis_depth: usize,
    /// Record the `$` that opened this substitution for source-range checks.
    start: usize,
    /// Track `case ... esac` nesting so a pattern terminator is not a closer.
    pub(super) case_nesting: CaseNesting,
    /// Record what the delimiter opened: a command list or an arithmetic body.
    kind: SubstitutionKind,
}

/// Identify a command-substitution delimiter in a script traversal.
enum ScriptCommandSubstitutionDelimiter {
    /// Begin a `$()` command substitution.
    Start,
    /// Open a grouped expression in the active command substitution.
    NestedOpen,
    /// Close a grouped expression or command substitution.
    Close,
}

impl ScriptCommandSubstitution {
    /// Initialise the outermost group of one command substitution.
    const fn new(start: usize, kind: SubstitutionKind) -> Self {
        Self {
            quote_context: ShellQuoteContext::Unquoted,
            parenthesis_depth: 1,
            start,
            case_nesting: CaseNesting::new(),
            kind,
        }
    }

    /// Report whether this substitution's body is an arithmetic expansion.
    pub(super) const fn is_arithmetic(&self) -> bool {
        matches!(self.kind, SubstitutionKind::Arithmetic)
    }

    /// Report whether a `)` at the cursor would end this substitution.
    const fn is_outermost_group(&self) -> bool {
        self.parenthesis_depth == 1
    }
}

impl ScriptSubstitutionTraversal<'_, '_> {
    /// Preserve `$()` delimiters while rejecting placeholders within their scope.
    pub(super) fn append_command_substitution_delimiter(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<Option<usize>, IrGenError> {
        let Some(delimiter) = self.classify_command_substitution_delimiter(pos, ch) else {
            return Ok(None);
        };
        match delimiter {
            ScriptCommandSubstitutionDelimiter::Start => {
                self.command_substitutions
                    .push(ScriptCommandSubstitution::new(
                        pos,
                        SubstitutionKind::at(self.chars, pos),
                    ));
                self.posix_lexical.enter_command_substitution_region();
                self.output.push_str("$(");
                Ok(Some(pos + 2))
            }
            ScriptCommandSubstitutionDelimiter::NestedOpen => {
                self.increment_command_substitution_parenthesis_depth();
                self.output.push(ch);
                Ok(Some(pos + 1))
            }
            ScriptCommandSubstitutionDelimiter::Close => self.close_script_substitution(pos, ch),
        }
    }

    /// Close one grouped expression or the command substitution itself.
    ///
    /// A `)` that terminates a `case` pattern is kept in the output without
    /// closing anything: the shell reads it as an operator, and only the later
    /// `)` that ends the substitution belongs to the surrounding word.
    ///
    /// # Errors
    ///
    /// Returns an invalid-command error when the `)` would close the region that
    /// opened an open backtick, because the saved quote context would then have
    /// no region to return to, and when a `case` body makes the comment rule
    /// ambiguous.
    fn close_script_substitution(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<Option<usize>, IrGenError> {
        if self.is_case_pattern_terminator() {
            self.output.push(ch);
            return Ok(Some(pos + 1));
        }
        if self.closes_backtick_owner() {
            return Err(invalid_command_error(
                self.template.to_owned(),
                RejectionReason::UnmatchedBacktick,
            ));
        }
        let boundary = self.boundary_at(pos);
        let Some(is_closed) = self.decrement_command_substitution_parenthesis_depth() else {
            return Ok(None);
        };
        if is_closed {
            reject_unplaced_case(SourceText::new(self.chars), &boundary)?;
            self.posix_lexical.leave_command_substitution_region();
            reject_ambiguous_case_comment(SourceText::new(self.chars), &boundary)?;
            self.posix_lexical.record_closed_command_substitution(pos);
        }
        self.output.push(ch);
        Ok(Some(pos + 1))
    }

    /// Describe the substitution boundary that the `)` at `pos` may close.
    ///
    /// The description is captured before the depth is decremented, because
    /// closing the substitution discards the state the report is built from.
    fn boundary_at(&self, pos: usize) -> SubstitutionBoundary {
        self.command_substitutions
            .last()
            .filter(|substitution| substitution.parenthesis_depth == 1)
            .map_or(
                SubstitutionBoundary {
                    pos,
                    start: None,
                    saw_case: false,
                    has_unplaced_case: false,
                },
                |substitution| SubstitutionBoundary {
                    pos,
                    start: Some(substitution.start),
                    saw_case: substitution.case_nesting.saw_case(),
                    has_unplaced_case: substitution.case_nesting.has_unplaced_case(),
                },
            )
    }

    /// Report whether the `)` at the cursor would close a backtick's owning region.
    ///
    /// The backtick saved the quote context of the region at its own depth, so
    /// closing that region leaves the record with nowhere to restore. Rejecting
    /// at the delimiter keeps the failure where the nesting first breaks instead
    /// of deferring it to the backtick reached later.
    fn closes_backtick_owner(&self) -> bool {
        let closes_substitution = self
            .command_substitutions
            .last()
            .is_some_and(ScriptCommandSubstitution::is_outermost_group);
        closes_substitution
            && self
                .backtick_region
                .is_some_and(|region| region.owner_depth == self.command_substitutions.len())
    }

    /// Record one source character for `case` nesting inside a substitution.
    ///
    /// Only the innermost substitution is updated: a `case` in an outer region
    /// cannot terminate a pattern of an inner one.
    pub(super) fn observe_case_word(&mut self, pos: usize, ch: char) {
        let is_unquoted = self.active_quote_context() == ShellQuoteContext::Unquoted;
        let next = self.chars.get(pos + 1).copied();
        if let Some(substitution) = self.command_substitutions.last_mut() {
            let observation =
                CaseObservation::new(ch, next, is_unquoted, substitution.parenthesis_depth);
            substitution.case_nesting.observe(observation);
        }
    }

    /// Tell the innermost substitution's tracker that an unquoted comment began.
    pub(super) fn begin_case_comment(&mut self) {
        if let Some(substitution) = self.command_substitutions.last_mut() {
            substitution.case_nesting.begin_comment();
        }
    }

    /// Tell the innermost substitution's tracker that a comment ended.
    pub(super) fn end_case_comment(&mut self) {
        if let Some(substitution) = self.command_substitutions.last_mut() {
            substitution.case_nesting.end_comment();
        }
    }

    /// Report whether the current `)` terminates a `case` pattern.
    ///
    /// While a `case` construct is open, a `)` at that construct's own depth
    /// cannot close the substitution: the shell reads it as a pattern
    /// terminator. The depth matters because a `case` may sit inside a nested
    /// group, where its terminator is one level below the substitution's.
    /// Keeping the substitution open makes the real closer the one that is
    /// checked.
    fn is_case_pattern_terminator(&self) -> bool {
        self.command_substitutions
            .last()
            .is_some_and(|substitution| {
                substitution
                    .case_nesting
                    .is_open_at(substitution.parenthesis_depth)
            })
    }

    /// Classify a character that delimits an active script command substitution.
    fn classify_command_substitution_delimiter(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Option<ScriptCommandSubstitutionDelimiter> {
        if self.starts_command_substitution(pos, ch) {
            return Some(ScriptCommandSubstitutionDelimiter::Start);
        }
        // A backtick body is its own command region. Its parentheses group
        // commands inside that region, so they must not move the depth of the
        // `$()` frame that owns the backtick: doing so would let a body `)`
        // stand in for the owner's closer and leave a later marker encoded for
        // a quoting the shell does not use.
        if self.is_backtick_body_text() {
            return None;
        }
        if self.opens_grouped_expression(pos, ch) {
            return Some(ScriptCommandSubstitutionDelimiter::NestedOpen);
        }
        if self.closes_command_substitution(ch) {
            return Some(ScriptCommandSubstitutionDelimiter::Close);
        }
        None
    }

    /// Report whether the `(` at `pos` opens a grouped expression.
    ///
    /// A process substitution raises the depth and names a header subject; a
    /// plain `(` is the optional pattern-list opener only when the tracker
    /// consumes it, and is a group otherwise.
    fn opens_grouped_expression(&mut self, pos: usize, ch: char) -> bool {
        if !self.is_unquoted_command_substitution() {
            return false;
        }
        if ch != '(' {
            return false;
        }
        if self.opens_process_substitution(pos) {
            self.observe_process_substitution();
            return true;
        }
        !self.consume_pattern_opener()
    }

    /// Report whether `ch` closes the active command substitution.
    fn closes_command_substitution(&self, ch: char) -> bool {
        self.is_unquoted_command_substitution() && ch == ')'
    }

    /// Report whether the `(` at `pos` follows `<` or `>`.
    ///
    /// That pair is a process substitution rather than Bash's optional
    /// pattern-list opener, so it always raises the grouping depth.
    fn opens_process_substitution(&self, pos: usize) -> bool {
        pos.checked_sub(1)
            .and_then(|previous| self.chars.get(previous))
            .is_some_and(|ch| matches!(ch, '<' | '>'))
    }

    /// Record a process substitution for the innermost `case` tracker.
    fn observe_process_substitution(&mut self) {
        if let Some(substitution) = self.command_substitutions.last_mut() {
            substitution
                .case_nesting
                .observe_process_substitution(substitution.parenthesis_depth);
        }
    }

    /// Report whether the cursor sits directly in an open backtick body.
    fn is_backtick_body_text(&self) -> bool {
        self.backtick_region
            .is_some_and(|region| region.owner_depth == self.command_substitutions.len())
    }

    /// Return whether `ch` opens an unquoted POSIX `$()` command substitution.
    fn starts_command_substitution(&self, pos: usize, ch: char) -> bool {
        matches!(
            (ch, self.chars.get(pos + 1), self.active_quote_context()),
            ('$', Some('('), context)
                if !matches!(
                    context,
                    ShellQuoteContext::SingleQuoted | ShellQuoteContext::AnsiC
                )
        )
    }

    /// Return whether the active command substitution is outside shell quotes.
    fn is_unquoted_command_substitution(&self) -> bool {
        self.is_in_command_substitution()
            && self.active_quote_context() == ShellQuoteContext::Unquoted
    }

    /// Consume a `(` at the active depth that opens a `case` pattern list.
    ///
    /// Bash accepts one optional `(` immediately before a pattern list, and
    /// that opener must not raise the grouping depth: the pattern terminator
    /// after it stands at the case's own depth, where the tracker recognises
    /// it. Reading the opener as a group would move the terminator one level
    /// deeper, where it closes nothing and leaves the construct in its pattern
    /// phase. Consuming it also records that the list has started, so an
    /// `esac` written as its first pattern stays pattern text.
    fn consume_pattern_opener(&mut self) -> bool {
        self.command_substitutions
            .last_mut()
            .is_some_and(|substitution| {
                substitution
                    .case_nesting
                    .consume_pattern_opener(substitution.parenthesis_depth)
            })
    }

    /// Increase the grouping depth in the innermost command substitution.
    fn increment_command_substitution_parenthesis_depth(&mut self) {
        if let Some(substitution) = self.command_substitutions.last_mut() {
            substitution.parenthesis_depth += 1;
        }
    }

    /// Close one grouping level and report whether the substitution ended.
    ///
    /// Only the `)` that leaves the substitution belongs to the outer word; a
    /// `)` closing an inner group is still a separator inside the substitution.
    fn decrement_command_substitution_parenthesis_depth(&mut self) -> Option<bool> {
        let substitution = self.command_substitutions.last_mut()?;
        substitution.parenthesis_depth -= 1;
        let is_closed = substitution.parenthesis_depth == 0;
        if is_closed {
            self.command_substitutions.pop();
        }
        Some(is_closed)
    }
}
// weave: run 'weave explain src/ir/cmd_interpolate/script_substitution/delimiters.rs' for per-hunk detail, 'weave check' to verify your resolution
