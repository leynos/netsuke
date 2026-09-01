//! Hold the state required for one-pass recipe placeholder substitution.

use super::{
    CommandBindings, IrGenError, QuoteContext,
    command_substitution::{CommandSubstitution, CommandSubstitutionDelimiter},
    find_substitution, invalid_command_error,
    posix_lexical::{PosixCharacter, PosixLexicalState},
};
use crate::recipe_shell::RecipeShell;

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

/// Track one-pass substitution while retaining the original error template.
///
/// This helper is private to `cmd_interpolate`: it groups immutable template
/// context with mutable output state so per-character processing avoids a wide
/// parameter list without re-scanning the template.
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
    /// Record the active POSIX quote context outside command substitutions.
    quote_context: QuoteContext,
    /// Retain nested `$()` parsing state where path insertion is refused.
    command_substitutions: Vec<CommandSubstitution>,
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
            quote_context: QuoteContext::Unquoted,
            command_substitutions: Vec::new(),
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
            return self.append_power_shell_character(pos, ch);
        }
        self.append_posix_character(pos, ch)
    }

    /// Preserve POSIX shell syntax while lowering context-safe recipe markers.
    fn append_posix_character(&mut self, pos: usize, ch: char) -> Result<usize, IrGenError> {
        let copied_inert_character = {
            let mut character = PosixCharacter {
                chars: self.chars,
                pos,
                ch,
                output: &mut self.output,
            };
            self.posix_lexical.append_inert_character(&mut character)
        };
        if copied_inert_character {
            return Ok(pos + 1);
        }
        if let Some(next) = self.append_escaped_character(pos, ch) {
            return Ok(next);
        }
        if self.matches_unquoted_quote_context() {
            let declaration_end = {
                let mut character = PosixCharacter {
                    chars: self.chars,
                    pos,
                    ch,
                    output: &mut self.output,
                };
                self.posix_lexical
                    .append_heredoc_declaration(&mut character, self.bindings)
            };
            if let Some(next) = declaration_end {
                return Ok(next);
            }
            if PosixLexicalState::starts_comment(self.chars, pos, ch) {
                self.posix_lexical.begin_comment();
                self.output.push(ch);
                return Ok(pos + 1);
            }
        }
        if ch == '`' && !self.matches_single_quote_context() {
            self.in_backticks ^= true;
            self.output.push(ch);
            return Ok(pos + 1);
        }
        let next = self.append_contextual_character(pos, ch, Self::posix_marker_protection)?;
        if ch == '\n' {
            self.posix_lexical.begin_pending_heredoc_after_newline(next);
        }
        Ok(next)
    }

    /// Preserve PowerShell text while lowering only manifest-owned markers.
    fn append_power_shell_character(&mut self, pos: usize, ch: char) -> Result<usize, IrGenError> {
        if let Some(next) = self.append_power_shell_escaped_character(pos, ch)? {
            return Ok(next);
        }
        if self.append_power_shell_single_quote_escape(pos, ch) {
            return Ok(pos + 2);
        }
        self.append_contextual_character(pos, ch, Self::power_shell_marker_protection)
    }

    /// Preserve shared shell syntax before lowering a marker for its active context.
    fn append_contextual_character(
        &mut self,
        pos: usize,
        ch: char,
        marker_protection: fn(&Self) -> MarkerProtection,
    ) -> Result<usize, IrGenError> {
        if let Some(next) = self.append_command_substitution_delimiter(pos, ch) {
            return Ok(next);
        }
        self.update_quote_context(ch);
        self.append_marker_for_context(pos, ch, marker_protection(self))
    }

    /// Classify POSIX backticks and command substitutions as protected regions.
    const fn posix_marker_protection(&self) -> MarkerProtection {
        MarkerProtection::from_protected_region(
            self.in_backticks || self.is_in_command_substitution(),
        )
    }

    /// Classify PowerShell quotes and command substitutions as protected regions.
    fn power_shell_marker_protection(&self) -> MarkerProtection {
        MarkerProtection::from_protected_region(
            !matches!(self.active_quote_context(), QuoteContext::Unquoted)
                || self.is_in_command_substitution(),
        )
    }

    /// Preserve a PowerShell backtick escape unless it would hide a recipe marker.
    fn append_power_shell_escaped_character(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<Option<usize>, IrGenError> {
        if ch != '`' || self.matches_single_quote_context() {
            return Ok(None);
        }
        if find_substitution(self.chars, pos + 1).is_some() {
            return Err(invalid_command_error(self.template.to_owned()));
        }
        let Some(next) = self.chars.get(pos + 1) else {
            return Ok(None);
        };
        self.output.push(ch);
        self.output.push(*next);
        Ok(Some(pos + 2))
    }

    /// Preserve a doubled PowerShell apostrophe inside its single-quoted literal.
    fn append_power_shell_single_quote_escape(&mut self, pos: usize, ch: char) -> bool {
        if !self.matches_single_quote_context() || !self.has_doubled_apostrophe(pos, ch) {
            return false;
        }
        self.output.push_str("''");
        true
    }

    /// Report whether `ch` begins a doubled PowerShell apostrophe escape.
    fn has_doubled_apostrophe(&self, pos: usize, ch: char) -> bool {
        ch == '\'' && self.chars.get(pos + 1) == Some(&'\'')
    }

    /// Preserve a POSIX escaped character without interpreting its quote meaning.
    fn append_escaped_character(&mut self, pos: usize, ch: char) -> Option<usize> {
        if ch != '\\' || self.matches_single_quote_context() {
            return None;
        }
        if find_substitution(self.chars, pos + 1).is_some() {
            return None;
        }
        let next = *self.chars.get(pos + 1)?;
        self.output.push(ch);
        self.output.push(next);
        Some(pos + 2)
    }

    /// Track `$()` nesting and preserve its delimiters before marker handling.
    fn append_command_substitution_delimiter(&mut self, pos: usize, ch: char) -> Option<usize> {
        match self.classify_command_substitution_delimiter(pos, ch)? {
            CommandSubstitutionDelimiter::Start => {
                self.command_substitutions.push(CommandSubstitution::new());
                self.output.push_str("$(");
                Some(pos + 2)
            }
            CommandSubstitutionDelimiter::NestedOpen => {
                self.increment_command_substitution_parenthesis_depth();
                self.output.push(ch);
                Some(pos + 1)
            }
            CommandSubstitutionDelimiter::Close => {
                self.decrement_command_substitution_parenthesis_depth();
                self.output.push(ch);
                Some(pos + 1)
            }
        }
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

    /// Report whether the traversal is currently enclosed by POSIX single quotes.
    fn matches_single_quote_context(&self) -> bool {
        matches!(self.active_quote_context(), QuoteContext::Single)
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
            (QuoteContext::Unquoted, '\'') => *quote_context = QuoteContext::Single,
            (QuoteContext::Unquoted, '"') => *quote_context = QuoteContext::Double,
            (QuoteContext::Single, '\'') | (QuoteContext::Double, '"') => {
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

    /// Close one grouping level and leave the substitution when its delimiter closes.
    fn decrement_command_substitution_parenthesis_depth(&mut self) {
        let Some(substitution) = self.command_substitutions.last_mut() else {
            return;
        };
        substitution.parenthesis_depth -= 1;
        if substitution.parenthesis_depth == 0 {
            self.command_substitutions.pop();
        }
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
