//! Recognise and track `$()` delimiters inside one POSIX or Bash script.
//!
//! The `)` that closes a command substitution stays part of the surrounding
//! word, so its position must survive into the next comment check. These
//! helpers own that nesting state, keeping the parent traversal within the
//! repository's module-size cap.

use super::super::command_substitution::{
    CaseNesting, SubstitutionBoundary, reject_ambiguous_case_comment,
};
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
    const fn new(start: usize) -> Self {
        Self {
            quote_context: ShellQuoteContext::Unquoted,
            parenthesis_depth: 1,
            start,
            case_nesting: CaseNesting::new(),
        }
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
                    .push(ScriptCommandSubstitution::new(pos));
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
    fn close_script_substitution(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<Option<usize>, IrGenError> {
        if self.is_case_pattern_terminator() {
            self.output.push(ch);
            return Ok(Some(pos + 1));
        }
        let boundary = self.boundary_at(pos);
        let Some(is_closed) = self.decrement_command_substitution_parenthesis_depth() else {
            return Ok(None);
        };
        if is_closed {
            reject_ambiguous_case_comment(self.chars, &boundary)?;
            self.closed_command_substitution = Some(pos);
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
            .map_or_else(
                || SubstitutionBoundary::new(pos, None, false),
                |substitution| {
                    SubstitutionBoundary::new(
                        pos,
                        Some(substitution.start),
                        substitution.case_nesting.saw_case(),
                    )
                },
            )
    }

    /// Record one source character for `case` nesting inside a substitution.
    ///
    /// Only the innermost substitution is updated: a `case` in an outer region
    /// cannot terminate a pattern of an inner one.
    pub(super) fn observe_case_word(&mut self, pos: usize, ch: char) {
        let is_unquoted = self.active_quote_context() == ShellQuoteContext::Unquoted;
        let next = self.chars.get(pos + 1).copied();
        if let Some(substitution) = self.command_substitutions.last_mut() {
            substitution.case_nesting.observe(ch, next, is_unquoted);
        }
    }

    /// Report whether the current `)` terminates a `case` pattern.
    ///
    /// While a `case` construct is open, a `)` at the substitution's own depth
    /// cannot close it: the shell reads it as a pattern terminator. Keeping the
    /// substitution open makes the real closer the one that is checked.
    fn is_case_pattern_terminator(&self) -> bool {
        self.command_substitutions
            .last()
            .is_some_and(|substitution| {
                substitution.parenthesis_depth == 1 && substitution.case_nesting.is_open()
            })
    }

    /// Classify a character that delimits an active script command substitution.
    fn classify_command_substitution_delimiter(
        &self,
        pos: usize,
        ch: char,
    ) -> Option<ScriptCommandSubstitutionDelimiter> {
        if self.starts_command_substitution(pos, ch) {
            return Some(ScriptCommandSubstitutionDelimiter::Start);
        }
        if self.is_unquoted_command_substitution() && ch == '(' {
            return Some(ScriptCommandSubstitutionDelimiter::NestedOpen);
        }
        if self.is_unquoted_command_substitution() && ch == ')' {
            return Some(ScriptCommandSubstitutionDelimiter::Close);
        }
        None
    }

    /// Return whether `ch` opens an unquoted POSIX `$()` command substitution.
    fn starts_command_substitution(&self, pos: usize, ch: char) -> bool {
        matches!(
            (ch, self.chars.get(pos + 1), self.active_quote_context()),
            ('$', Some('('), context) if context != ShellQuoteContext::SingleQuoted
        )
    }

    /// Return whether the active command substitution is outside shell quotes.
    fn is_unquoted_command_substitution(&self) -> bool {
        self.is_in_command_substitution()
            && self.active_quote_context() == ShellQuoteContext::Unquoted
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
