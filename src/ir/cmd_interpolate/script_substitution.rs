//! Interpolate script-specific recipe placeholders with POSIX lexical context.
//!
//! Keeps multi-line script syntax, comments, and heredoc bodies intact while
//! lowering Netsuke-owned placeholders only where shell quoting can preserve
//! their path semantics.

use super::{
    CommandBindings, IrGenError, Placeholder, QuoteContext, find_script_substitution,
    find_substitution, invalid_command_error,
    posix_lexical::{PosixCharacter, PosixLexicalState},
};

/// Track quote context while interpolating one shell script.
pub(super) struct ScriptSubstitutionTraversal<'template, 'bindings> {
    /// Preserve the source text for interpolation diagnostics.
    template: &'template str,
    /// Retain the single character buffer traversed by the pass.
    chars: &'template [char],
    /// Reuse shell-specific path substitutions prepared for the enclosing recipe.
    bindings: &'bindings CommandBindings,
    /// Accumulate substituted output without a second traversal.
    output: String,
    /// Record the active ordinary POSIX shell quoting mode.
    quote_context: ShellQuoteContext,
    /// Restore the outer quoting mode after a backtick command substitution.
    backtick_outer_context: Option<ShellQuoteContext>,
    /// Track POSIX comments and heredocs whose text must remain lexically inert.
    posix_lexical: PosixLexicalState,
    /// Preserve a character escaped by a preceding backslash.
    is_escaped: bool,
    /// Retain nested command substitutions whose markers must be rejected.
    command_substitutions: Vec<ScriptCommandSubstitution>,
}

/// Describe the quoting mode that controls placeholder substitution.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ShellQuoteContext {
    /// Permit direct word-quoted substitution.
    Unquoted,
    /// Use the binding encoded for an existing POSIX single-quoted literal.
    SingleQuoted,
    /// Temporarily close and reopen double quotes around a word-quoted path.
    DoubleQuoted,
}

/// Track one nested POSIX command substitution while traversing a script.
struct ScriptCommandSubstitution {
    /// Record the quote context local to this command substitution.
    quote_context: ShellQuoteContext,
    /// Count nested parenthesised groups until the closing delimiter.
    parenthesis_depth: usize,
}

impl ScriptCommandSubstitution {
    /// Initialise the outermost group of one command substitution.
    const fn new() -> Self {
        Self {
            quote_context: ShellQuoteContext::Unquoted,
            parenthesis_depth: 1,
        }
    }
}

impl<'template, 'bindings> ScriptSubstitutionTraversal<'template, 'bindings> {
    /// Initialise a quote-aware traversal for one script template.
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
            quote_context: ShellQuoteContext::Unquoted,
            backtick_outer_context: None,
            posix_lexical: PosixLexicalState::new(),
            is_escaped: false,
            command_substitutions: Vec::new(),
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
        if self.is_escaped {
            self.output.push(ch);
            self.is_escaped = false;
            return Ok(pos + 1);
        }
        if self.should_escape_next(pos, ch) {
            self.output.push(ch);
            self.is_escaped = true;
            return Ok(pos + 1);
        }
        if let Some(next) = self.append_unquoted_posix_lexical_character(pos, ch) {
            return Ok(next);
        }
        if self.starts_comment(pos, ch) {
            self.output.push(ch);
            self.posix_lexical.begin_comment();
            return Ok(pos + 1);
        }
        if let Some(next) = self.append_command_substitution_delimiter(pos, ch) {
            return Ok(next);
        }
        if self.update_quote_context(ch) {
            self.output.push(ch);
            return Ok(pos + 1);
        }

        let substitution = find_script_substitution(self.chars, pos);
        let next = self.append_substitution_or_character(pos, ch, substitution)?;
        if ch == '\n' {
            self.posix_lexical.begin_pending_heredoc_after_newline(next);
        }
        Ok(next)
    }

    /// Preserve unquoted POSIX heredoc declarations before their bodies become inert.
    fn append_unquoted_posix_lexical_character(&mut self, pos: usize, ch: char) -> Option<usize> {
        if self.active_quote_context() != ShellQuoteContext::Unquoted
            || self.backtick_outer_context.is_some()
        {
            return None;
        }
        let mut character = PosixCharacter {
            chars: self.chars,
            pos,
            ch,
            output: &mut self.output,
        };
        self.posix_lexical
            .append_heredoc_declaration(&mut character, self.bindings)
    }

    /// Return whether `ch` begins a POSIX shell comment in ordinary text.
    fn starts_comment(&self, pos: usize, ch: char) -> bool {
        self.active_quote_context() == ShellQuoteContext::Unquoted
            && self.backtick_outer_context.is_none()
            && PosixLexicalState::starts_comment(self.chars, pos, ch)
    }

    /// Return whether a backslash escapes the following script character.
    fn should_escape_next(&self, pos: usize, ch: char) -> bool {
        ch == '\\'
            && self.active_quote_context() != ShellQuoteContext::SingleQuoted
            && find_substitution(self.chars, pos + 1).is_none()
    }

    /// Update quoting state and return whether `ch` was a quote delimiter.
    fn update_quote_context(&mut self, ch: char) -> bool {
        if let Some(outer_context) = self.backtick_outer_context {
            if ch == '`' {
                self.quote_context = outer_context;
                self.backtick_outer_context = None;
                return true;
            }
            return false;
        }

        let quote_context = self.active_quote_context_mut();
        match (*quote_context, ch) {
            (ShellQuoteContext::Unquoted, '\'') => {
                *quote_context = ShellQuoteContext::SingleQuoted;
                true
            }
            (ShellQuoteContext::Unquoted, '"') => {
                *quote_context = ShellQuoteContext::DoubleQuoted;
                true
            }
            (ShellQuoteContext::SingleQuoted, '\'') | (ShellQuoteContext::DoubleQuoted, '"') => {
                *quote_context = ShellQuoteContext::Unquoted;
                true
            }
            (ShellQuoteContext::Unquoted | ShellQuoteContext::DoubleQuoted, '`') => {
                self.backtick_outer_context = Some(*quote_context);
                true
            }
            _ => false,
        }
    }

    /// Append a safe replacement or reject a placeholder in a protected region.
    fn append_substitution_or_character(
        &mut self,
        pos: usize,
        ch: char,
        substitution: Option<(Placeholder, usize)>,
    ) -> Result<usize, IrGenError> {
        let Some((placeholder, skip)) = substitution else {
            self.output.push(ch);
            return Ok(pos + 1);
        };
        if self.backtick_outer_context.is_some() || self.is_in_command_substitution() {
            return Err(invalid_command_error(self.template.to_owned()));
        }
        let context = match self.active_quote_context() {
            ShellQuoteContext::Unquoted => QuoteContext::Unquoted,
            ShellQuoteContext::SingleQuoted => QuoteContext::Single,
            ShellQuoteContext::DoubleQuoted => QuoteContext::Double,
        };
        self.output
            .push_str(self.bindings.substitution(placeholder, context));
        Ok(pos + skip)
    }

    /// Preserve `$()` delimiters while rejecting placeholders within their scope.
    fn append_command_substitution_delimiter(&mut self, pos: usize, ch: char) -> Option<usize> {
        if self.starts_command_substitution(pos, ch) {
            self.command_substitutions
                .push(ScriptCommandSubstitution::new());
            self.output.push_str("$(");
            return Some(pos + 2);
        }
        if self.is_unquoted_command_substitution() && ch == '(' {
            if let Some(substitution) = self.command_substitutions.last_mut() {
                substitution.parenthesis_depth += 1;
            }
            self.output.push(ch);
            return Some(pos + 1);
        }
        if self.is_unquoted_command_substitution() && ch == ')' {
            let substitution = self.command_substitutions.last_mut()?;
            substitution.parenthesis_depth -= 1;
            if substitution.parenthesis_depth == 0 {
                self.command_substitutions.pop();
            }
            self.output.push(ch);
            return Some(pos + 1);
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

    /// Return the quote context of the innermost command substitution when active.
    fn active_quote_context(&self) -> ShellQuoteContext {
        self.command_substitutions
            .last()
            .map_or(self.quote_context, |substitution| {
                substitution.quote_context
            })
    }

    /// Borrow the quote context of the innermost active shell region.
    fn active_quote_context_mut(&mut self) -> &mut ShellQuoteContext {
        match self.command_substitutions.last_mut() {
            Some(substitution) => &mut substitution.quote_context,
            None => &mut self.quote_context,
        }
    }

    /// Report whether a `$()` command substitution is active.
    const fn is_in_command_substitution(&self) -> bool {
        !self.command_substitutions.is_empty()
    }

    /// Finish the traversal and return the generated shell text.
    pub(super) fn finish(self) -> String {
        self.output
    }
}
