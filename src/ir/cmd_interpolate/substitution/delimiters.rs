//! Recognise and track `$()` delimiters inside one recipe substitution.
//!
//! The `)` that closes a command substitution stays part of the surrounding
//! word, so its position must survive into the next boundary check. These
//! helpers own that nesting state, the `case` observations that qualify it,
//! and the fail-closed checks applied when a substitution closes. Keeping them
//! beside the traversal rather than inside it holds the parent module within
//! the repository's module-size cap.

use super::super::case_nesting::CaseObservation;
use super::super::command_substitution::{
    CommandSubstitution, CommandSubstitutionDelimiter, SourceText, SubstitutionBoundary,
    SubstitutionStart, reject_ambiguous_case_comment, reject_unplaced_case,
};
use super::super::posix_lexical::SubstitutionKind;
use super::{IrGenError, SubstitutionTraversal};
use crate::recipe_shell::RecipeShell;

impl SubstitutionTraversal<'_, '_> {
    /// Track `$()` nesting and preserve its delimiters before marker handling.
    pub(super) fn append_command_substitution_delimiter(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<Option<usize>, IrGenError> {
        let Some(delimiter) = self.classify_command_substitution_delimiter(pos, ch) else {
            return Ok(None);
        };
        match delimiter {
            CommandSubstitutionDelimiter::Start => {
                self.command_substitutions
                    .push(CommandSubstitution::new(SubstitutionStart {
                        pos,
                        kind: SubstitutionKind::at(self.chars, pos),
                    }));
                self.posix_lexical.enter_command_substitution_region();
                self.output.push_str("$(");
                Ok(Some(pos + 2))
            }
            CommandSubstitutionDelimiter::NestedOpen => {
                self.increment_command_substitution_parenthesis_depth();
                self.output.push(ch);
                Ok(Some(pos + 1))
            }
            CommandSubstitutionDelimiter::Close => self.close_substitution(pos, ch),
        }
    }

    /// Close one substitution level and reject an ambiguous `case` boundary.
    fn close_substitution(&mut self, pos: usize, ch: char) -> Result<Option<usize>, IrGenError> {
        if self.is_case_pattern_terminator() {
            self.output.push(ch);
            return Ok(Some(pos + 1));
        }
        let boundary = self.boundary_at(pos);
        if self.decrement_command_substitution_parenthesis_depth() {
            reject_unplaced_case(SourceText::new(self.chars), &boundary)?;
            self.posix_lexical.leave_command_substitution_region();
            self.reject_ambiguous_boundary(&boundary)?;
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

    /// Reject a `)#` boundary that the active shell cannot disambiguate.
    ///
    /// PowerShell has no `case` patterns, so its ambiguity only arises in an
    /// unquoted region; POSIX and Bash always check.
    fn reject_ambiguous_boundary(&self, boundary: &SubstitutionBoundary) -> Result<(), IrGenError> {
        let is_checked =
            self.bindings.shell != RecipeShell::PowerShell || self.matches_unquoted_quote_context();
        if is_checked {
            reject_ambiguous_case_comment(SourceText::new(self.chars), boundary)?;
        }
        Ok(())
    }

    /// Record one source character for `case` nesting inside a substitution.
    ///
    /// Only the innermost substitution is updated: a `case` in an outer region
    /// cannot terminate a pattern of an inner one.
    pub(super) fn observe_case_word(&mut self, pos: usize, ch: char) {
        let is_unquoted = self.matches_unquoted_quote_context();
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

    /// Classify a character that delimits an active command substitution.
    fn classify_command_substitution_delimiter(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Option<CommandSubstitutionDelimiter> {
        if !self.matches_single_quote_context() && self.starts_command_substitution(pos, ch) {
            return Some(CommandSubstitutionDelimiter::Start);
        }
        if self.starts_nested_command_substitution_parenthesis(ch) {
            return Some(CommandSubstitutionDelimiter::NestedOpen);
        }
        if !self.matches_single_quote_context() && self.ends_command_substitution(ch) {
            return Some(CommandSubstitutionDelimiter::Close);
        }
        None
    }

    /// Report whether `ch` starts a POSIX or PowerShell `$()` expression.
    fn starts_command_substitution(&self, pos: usize, ch: char) -> bool {
        ch == '$' && self.chars.get(pos + 1) == Some(&'(')
    }

    /// Report whether the innermost active substitution is arithmetic.
    ///
    /// An arithmetic body evaluates numbers, so `<<` there is a shift
    /// operator. Heredoc tracking must not run inside one: the tracker would
    /// invent a body the shell never starts and end it at a line that is still
    /// inside an expansion.
    pub(super) fn is_in_arithmetic_expansion(&self) -> bool {
        self.command_substitutions
            .last()
            .is_some_and(CommandSubstitution::is_arithmetic)
    }

    /// Report whether `ch` opens a grouped expression inside a `$()` region.
    ///
    /// Bash accepts one optional `(` immediately before a `case` pattern list,
    /// and that opener must not raise the grouping depth: the pattern
    /// terminator after it stands at the case's own depth, where the tracker
    /// recognises it. Reading the opener as a group would move the terminator
    /// one level deeper, where it closes nothing and leaves the construct in
    /// its pattern phase.
    fn starts_nested_command_substitution_parenthesis(&mut self, ch: char) -> bool {
        let opens_group =
            self.is_in_command_substitution() && self.matches_unquoted_quote_context();
        let is_parenthesis = opens_group && ch == '(';
        is_parenthesis && !self.consume_pattern_opener()
    }

    /// Consume a `(` at the active depth that opens a `case` pattern list.
    fn consume_pattern_opener(&mut self) -> bool {
        self.command_substitutions
            .last_mut()
            .is_some_and(|substitution| {
                substitution
                    .case_nesting
                    .consume_pattern_opener(substitution.parenthesis_depth)
            })
    }

    /// Report whether `ch` closes the current `$()` expression.
    fn ends_command_substitution(&self, ch: char) -> bool {
        self.is_in_command_substitution() && self.matches_unquoted_quote_context() && ch == ')'
    }

    /// Report whether the traversal is currently inside a command substitution.
    pub(super) const fn is_in_command_substitution(&self) -> bool {
        !self.command_substitutions.is_empty()
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
    /// A `true` result also marks the point where the caller ends the region's
    /// `${ }` bookkeeping, because the region itself has ended here.
    fn decrement_command_substitution_parenthesis_depth(&mut self) -> bool {
        let Some(substitution) = self.command_substitutions.last_mut() else {
            return false;
        };
        substitution.parenthesis_depth -= 1;
        let is_closed = substitution.parenthesis_depth == 0;
        if is_closed {
            self.command_substitutions.pop();
        }
        is_closed
    }
}
