//! Hold the state required for one-pass recipe placeholder substitution.

use super::{CommandBindings, IrGenError, QuoteContext, find_substitution, invalid_command_error};
use crate::recipe_shell::RecipeShell;

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
    /// Record nested `$()` regions where path insertion is deliberately refused.
    command_substitution_depth: usize,
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
            command_substitution_depth: 0,
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
        if let Some(next) = self.append_escaped_character(pos, ch) {
            return Ok(next);
        }
        if ch == '`' && !self.matches_single_quote_context() {
            self.in_backticks ^= true;
            self.output.push(ch);
            return Ok(pos + 1);
        }
        if let Some(next) = self.append_command_substitution_delimiter(pos, ch) {
            return Ok(next);
        }
        self.update_quote_context(ch);
        let substitution = find_substitution(self.chars, pos);
        if self.in_backticks || self.command_substitution_depth > 0 {
            return self.append_protected_character(pos, ch, substitution);
        }
        Ok(self.append_unprotected_character(pos, ch, substitution))
    }

    /// Preserve PowerShell text while lowering only manifest-owned markers.
    fn append_power_shell_character(&mut self, pos: usize, ch: char) -> Result<usize, IrGenError> {
        if let Some(next) = self.append_power_shell_escaped_character(pos, ch) {
            return Ok(next);
        }
        if self.append_power_shell_single_quote_escape(pos, ch) {
            return Ok(pos + 2);
        }
        if let Some(next) = self.append_command_substitution_delimiter(pos, ch) {
            return Ok(next);
        }
        self.update_quote_context(ch);
        let substitution = find_substitution(self.chars, pos);
        if !matches!(self.quote_context, QuoteContext::Unquoted)
            || self.command_substitution_depth > 0
        {
            return self.append_protected_character(pos, ch, substitution);
        }
        Ok(self.append_unprotected_character(pos, ch, substitution))
    }

    /// Preserve a PowerShell backtick escape without interpreting its next character.
    fn append_power_shell_escaped_character(&mut self, pos: usize, ch: char) -> Option<usize> {
        if ch != '`' || self.matches_single_quote_context() {
            return None;
        }
        self.output.push(ch);
        let next = self.chars.get(pos + 1)?;
        self.output.push(*next);
        Some(pos + 2)
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
        self.output.push(ch);
        let next = self.chars.get(pos + 1)?;
        self.output.push(*next);
        Some(pos + 2)
    }

    /// Track `$()` nesting and preserve its delimiters before marker handling.
    fn append_command_substitution_delimiter(&mut self, pos: usize, ch: char) -> Option<usize> {
        if !self.matches_single_quote_context() && self.starts_command_substitution(pos, ch) {
            self.command_substitution_depth += 1;
            self.output.push_str("$(");
            return Some(pos + 2);
        }
        if !self.matches_single_quote_context() && self.ends_command_substitution(ch) {
            self.command_substitution_depth -= 1;
            self.output.push(ch);
            return Some(pos + 1);
        }
        None
    }

    /// Report whether `ch` starts a POSIX or PowerShell `$()` expression.
    fn starts_command_substitution(&self, pos: usize, ch: char) -> bool {
        ch == '$' && self.chars.get(pos + 1) == Some(&'(')
    }

    /// Report whether `ch` closes the current `$()` expression.
    const fn ends_command_substitution(&self, ch: char) -> bool {
        ch == ')' && self.command_substitution_depth > 0
    }

    /// Change POSIX quote state after retaining a delimiter in the output.
    const fn update_quote_context(&mut self, ch: char) {
        match (self.quote_context, ch) {
            (QuoteContext::Unquoted, '\'') => self.quote_context = QuoteContext::Single,
            (QuoteContext::Unquoted, '"') => self.quote_context = QuoteContext::Double,
            (QuoteContext::Single, '\'') | (QuoteContext::Double, '"') => {
                self.quote_context = QuoteContext::Unquoted;
            }
            _ => {}
        }
    }

    /// Report whether the traversal is currently enclosed by POSIX single quotes.
    const fn matches_single_quote_context(&self) -> bool {
        matches!(self.quote_context, QuoteContext::Single)
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
        self.output
            .push_str(self.bindings.substitution(placeholder, self.quote_context));
        pos + skip
    }

    /// Finish the traversal and return the generated shell text.
    pub(super) fn finish(self) -> String {
        self.output
    }
}
