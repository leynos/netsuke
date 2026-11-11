//! Hold the state required for one-pass recipe placeholder substitution.

use super::{
    CommandBindings, IrGenError, QuoteContext,
    command_substitution::{
        CaseObservation, CommandSubstitution, CommandSubstitutionDelimiter, SubstitutionBoundary,
        reject_ambiguous_case_comment,
    },
    find_substitution, invalid_command_error,
    posix_lexical::PosixLexicalState,
};
use crate::recipe_shell::RecipeShell;

mod posix;
mod powershell;

use powershell::{PowerShellLexicalState, PowerShellSourceCharacter, PowerShellStopParsingState};

/// Classify whether the active shell region permits recipe-marker lowering.
#[derive(Clone, Copy)]
enum MarkerProtection {
    /// Reject a marker because the shell region cannot safely encode it.
    Protected,
    /// Lower a marker using the current shell quote context.
    Unprotected,
}

impl MarkerProtection {
    /// Classify a shell-context predicate without exposing a Boolean at call sites.
    const fn from_protected_region(is_protected: bool) -> Self {
        if is_protected {
            Self::Protected
        } else {
            Self::Unprotected
        }
    }
}

/// Track one-pass substitution with template context and mutable output state.
///
/// Keep this helper private to avoid a wide per-character parameter list.
pub(super) struct SubstitutionTraversal<'template, 'bindings> {
    /// Preserve the source text for interpolation diagnostics.
    template: &'template str,
    /// Retain the single character buffer traversed by the pass.
    chars: &'template [char],
    /// Reuse shell-specific path substitutions prepared for the enclosing action.
    bindings: &'bindings CommandBindings,
    /// Accumulate substituted output without a second traversal.
    output: String,
    /// Record whether the current position is protected by backticks.
    in_backticks: bool,
    /// Track PowerShell here-strings, comments, and fail-closed lexical state.
    power_shell_lexical: PowerShellLexicalState,
    /// Track native-command arguments following PowerShell's `--%` token.
    power_shell_stop_parsing: PowerShellStopParsingState,
    /// Record the active shell quote context outside command substitutions.
    quote_context: QuoteContext,
    /// Retain nested `$()` parsing state where path insertion is refused.
    command_substitutions: Vec<CommandSubstitution>,
    /// Record the `)` that closed the most recent command substitution.
    closed_command_substitution: Option<usize>,
    /// Track POSIX comments and heredocs whose text must remain lexically inert.
    posix_lexical: PosixLexicalState,
}

impl<'template, 'bindings> SubstitutionTraversal<'template, 'bindings> {
    /// Initialise a traversal for one template and its placeholder bindings.
    pub(super) fn new(
        template: &'template str,
        chars: &'template [char],
        bindings: &'bindings CommandBindings,
    ) -> Self {
        Self {
            template,
            chars,
            bindings,
            output: String::with_capacity(template.len()),
            in_backticks: false,
            power_shell_lexical: PowerShellLexicalState::new(),
            power_shell_stop_parsing: PowerShellStopParsingState::new(),
            quote_context: QuoteContext::Unquoted,
            command_substitutions: Vec::new(),
            closed_command_substitution: None,
            posix_lexical: PosixLexicalState::new(),
        }
    }

    /// Append the substitution or source character beginning at `pos`.
    pub(super) fn append_substitution_at_position(
        &mut self,
        pos: usize,
    ) -> Result<usize, IrGenError> {
        let ch = *self
            .chars
            .get(pos)
            .ok_or_else(|| invalid_command_error(self.template.to_owned()))?;
        if self.bindings.shell == RecipeShell::PowerShell {
            let character = PowerShellSourceCharacter::new(self.chars, pos)
                .ok_or_else(|| invalid_command_error(self.template.to_owned()))?;
            return self.append_power_shell_character(character);
        }
        self.append_posix_character(pos, ch)
    }

    /// Preserve shared shell syntax before lowering a marker for its active context.
    fn append_contextual_character(
        &mut self,
        pos: usize,
        ch: char,
        marker_protection: fn(&Self, usize) -> MarkerProtection,
    ) -> Result<usize, IrGenError> {
        if let Some(next) = self.append_command_substitution_delimiter(pos, ch)? {
            return Ok(next);
        }
        self.update_quote_context(ch);
        self.append_marker_for_context(pos, ch, marker_protection(self, pos))
    }
    /// Track `$()` nesting and preserve its delimiters before marker handling.
    fn append_command_substitution_delimiter(
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
                    .push(CommandSubstitution::new(pos));
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

    /// Record one source character for `case` nesting inside a substitution.
    ///
    /// Only the innermost substitution is updated: a `case` in an outer region
    /// cannot terminate a pattern of an inner one.
    fn observe_case_word(&mut self, pos: usize, ch: char) {
        let is_unquoted = self.matches_unquoted_quote_context();
        let next = self.chars.get(pos + 1).copied();
        if let Some(substitution) = self.command_substitutions.last_mut() {
            let observation =
                CaseObservation::new(ch, next, is_unquoted, substitution.parenthesis_depth);
            substitution.case_nesting.observe(observation);
        }
    }

    /// Tell the innermost substitution's tracker that an unquoted comment began.
    fn begin_case_comment(&mut self) {
        if let Some(substitution) = self.command_substitutions.last_mut() {
            substitution.case_nesting.begin_comment();
        }
    }

    /// Tell the innermost substitution's tracker that a comment ended.
    fn end_case_comment(&mut self) {
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

    /// Close one substitution level and reject an ambiguous `case` boundary.
    fn close_substitution(&mut self, pos: usize, ch: char) -> Result<Option<usize>, IrGenError> {
        if self.is_case_pattern_terminator() {
            self.output.push(ch);
            return Ok(Some(pos + 1));
        }
        let boundary = self.boundary_at(pos);
        if self.decrement_command_substitution_parenthesis_depth() {
            self.reject_ambiguous_boundary(&boundary)?;
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

    /// Reject a `)#` boundary that the active shell cannot disambiguate.
    ///
    /// PowerShell has no `case` patterns, so its ambiguity only arises in an
    /// unquoted region; POSIX and Bash always check.
    fn reject_ambiguous_boundary(&self, boundary: &SubstitutionBoundary) -> Result<(), IrGenError> {
        let is_checked =
            self.bindings.shell != RecipeShell::PowerShell || self.matches_unquoted_quote_context();
        if is_checked {
            reject_ambiguous_case_comment(self.chars, boundary)?;
        }
        Ok(())
    }

    /// Classify a character that delimits an active command substitution.
    fn classify_command_substitution_delimiter(
        &self,
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

    /// Report whether `ch` opens a grouped expression inside a `$()` region.
    fn starts_nested_command_substitution_parenthesis(&self, ch: char) -> bool {
        self.is_in_command_substitution() && self.matches_unquoted_quote_context() && ch == '('
    }

    /// Report whether `ch` closes the current `$()` expression.
    fn ends_command_substitution(&self, ch: char) -> bool {
        self.is_in_command_substitution() && self.matches_unquoted_quote_context() && ch == ')'
    }

    /// Change POSIX quote state after retaining a delimiter in the output.
    fn update_quote_context(&mut self, ch: char) {
        let quote_context = self.active_quote_context_mut();
        Self::update_quote_context_for_region(quote_context, ch);
    }

    /// Report whether the traversal is currently enclosed by single quotes.
    fn matches_single_quote_context(&self) -> bool {
        matches!(self.active_quote_context(), QuoteContext::Single(_))
    }

    /// Report whether the active shell region is outside both kinds of quotes.
    fn matches_unquoted_quote_context(&self) -> bool {
        matches!(self.active_quote_context(), QuoteContext::Unquoted)
    }

    /// Report whether the traversal is currently inside a command substitution.
    const fn is_in_command_substitution(&self) -> bool {
        !self.command_substitutions.is_empty()
    }

    /// Return the quote state used by the innermost active shell region.
    fn active_quote_context(&self) -> QuoteContext {
        self.command_substitutions
            .last()
            .map_or(self.quote_context, |substitution| {
                substitution.quote_context
            })
    }

    /// Return the quote state storage used by the innermost active shell region.
    fn active_quote_context_mut(&mut self) -> &mut QuoteContext {
        match self.command_substitutions.last_mut() {
            Some(substitution) => &mut substitution.quote_context,
            None => &mut self.quote_context,
        }
    }

    /// Update the quote state of one shell region after copying its source character.
    const fn update_quote_context_for_region(quote_context: &mut QuoteContext, ch: char) {
        match (*quote_context, ch) {
            (QuoteContext::Unquoted, '\'') => *quote_context = QuoteContext::Single('\''),
            (QuoteContext::Unquoted, '"') => *quote_context = QuoteContext::Double('"'),
            (QuoteContext::Single(quote) | QuoteContext::Double(quote), character)
                if quote == character =>
            {
                *quote_context = QuoteContext::Unquoted;
            }
            _ => {}
        }
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

    /// Lower a marker when its current shell context permits substitution.
    fn append_marker_for_context(
        &mut self,
        pos: usize,
        ch: char,
        protection: MarkerProtection,
    ) -> Result<usize, IrGenError> {
        let substitution = find_substitution(self.chars, pos);
        if matches!(protection, MarkerProtection::Protected) {
            return self.append_protected_character(pos, ch, substitution);
        }
        Ok(self.append_unprotected_character(pos, ch, substitution))
    }

    /// Append a character protected by backticks or reject its placeholder.
    fn append_protected_character(
        &mut self,
        pos: usize,
        ch: char,
        substitution: Option<(super::Placeholder, usize)>,
    ) -> Result<usize, IrGenError> {
        if substitution.is_some() {
            return Err(invalid_command_error(self.template.to_owned()));
        }
        self.output.push(ch);
        Ok(pos + 1)
    }

    /// Append an unprotected replacement or the original character.
    fn append_unprotected_character(
        &mut self,
        pos: usize,
        ch: char,
        substitution: Option<(super::Placeholder, usize)>,
    ) -> usize {
        let Some((placeholder, skip)) = substitution else {
            self.output.push(ch);
            return pos + 1;
        };
        self.output.push_str(
            self.bindings
                .substitution(placeholder, self.active_quote_context()),
        );
        pos + skip
    }

    /// Finish the traversal and return the generated shell text.
    pub(super) fn finish(self) -> String {
        self.output
    }
}
