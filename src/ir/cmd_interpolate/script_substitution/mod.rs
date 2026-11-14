//! Interpolate POSIX and Bash script placeholders with script-specific context.
//!
//! Keeps multi-line script syntax, comments, and heredoc bodies intact while
//! lowering Netsuke-owned placeholders where shell quoting can preserve their
//! path semantics. PowerShell scripts use `SubstitutionTraversal` instead.

use super::{
    CommandBindings, IrGenError, Placeholder, QuoteContext, find_script_substitution,
    invalid_command_error,
    posix_lexical::{PosixCharacter, PosixLexicalState, SourceChar, backslash_escapes_marker},
};

mod delimiters;

mod heredoc;

use delimiters::ScriptCommandSubstitution;

/// Track quote context while interpolating one POSIX or Bash script.
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
        if let Some(next) = self.append_non_substitution_character(pos, ch)? {
            return Ok(next);
        }

        let substitution = find_script_substitution(self.chars, pos);
        let next = self.append_substitution_or_character(pos, ch, substitution)?;
        if ch == '\n' {
            self.posix_lexical.begin_pending_heredoc_after_newline();
        }
        Ok(next)
    }

    /// Process source characters that do not require placeholder substitution.
    ///
    /// # Errors
    ///
    /// Returns an invalid-command error when a heredoc delimiter or an escape
    /// would modify a marker lowered immediately after it.
    fn append_non_substitution_character(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<Option<usize>, IrGenError> {
        if let Some(next) = self.copy_inert_character(pos, ch) {
            return Ok(Some(next));
        }
        if self.is_escaped {
            self.output.push(ch);
            self.is_escaped = false;
            return Ok(Some(pos + 1));
        }
        self.observe_case_word(pos, ch);
        if self.starts_escape(pos, ch)? {
            self.output.push(ch);
            self.is_escaped = true;
            return Ok(Some(pos + 1));
        }
        if let Some(next) = self.append_unquoted_posix_lexical_character(pos, ch)? {
            return Ok(Some(next));
        }
        self.append_lexical_character(pos, ch)
    }

    /// Copy a character the scanner treats as inert, ending any comment it closes.
    ///
    /// Comment text and heredoc bodies are copied without inspection, so the
    /// traversal is the only part of the pass that can tell the tracker which
    /// newline ended a comment.
    fn copy_inert_character(&mut self, pos: usize, ch: char) -> Option<usize> {
        let ends_comment = self.posix_lexical.in_comment() && ch == '\n';
        let mut character = PosixCharacter {
            chars: self.chars,
            pos,
            ch,
            output: &mut self.output,
        };
        if !self.posix_lexical.append_inert_character(&mut character) {
            return None;
        }
        if ends_comment {
            self.end_case_comment();
        }
        Some(pos + 1)
    }

    /// Copy a character that opens a comment, a substitution, or a quote region.
    ///
    /// # Errors
    ///
    /// Returns an invalid-command error when a nested command substitution
    /// would lower a marker in a context its encoding cannot preserve.
    fn append_lexical_character(
        &mut self,
        pos: usize,
        ch: char,
    ) -> Result<Option<usize>, IrGenError> {
        if self.starts_comment(pos, ch) {
            self.output.push(ch);
            self.posix_lexical.begin_comment();
            self.begin_case_comment();
            return Ok(Some(pos + 1));
        }
        if let Some(next) = self.append_command_substitution_delimiter(pos, ch)? {
            return Ok(Some(next));
        }
        if self.update_quote_context(ch) {
            self.output.push(ch);
            return Ok(Some(pos + 1));
        }
        Ok(None)
    }

    /// Return whether `ch` begins a POSIX shell comment in ordinary text.
    ///
    /// An arithmetic expansion is not ordinary text: `#` there is arithmetic
    /// syntax, and a comment started inside `$(( ))` would swallow the rest of
    /// the line the shell still evaluates. The command traversal already
    /// suspends comment detection inside arithmetic, so this guard keeps both
    /// traversals on the same contract.
    fn starts_comment(&self, pos: usize, ch: char) -> bool {
        self.is_plain_script_text()
            && !self.is_in_arithmetic_expansion()
            && self
                .posix_lexical
                .starts_comment(self.chars, pos, SourceChar::new(ch))
    }

    /// Report whether a backslash begins an escape of the following character.
    ///
    /// # Errors
    ///
    /// Returns an invalid-command error when the escape would modify a marker
    /// lowered immediately after it.
    fn starts_escape(&self, pos: usize, ch: char) -> Result<bool, IrGenError> {
        if ch != '\\' || self.active_quote_context() == ShellQuoteContext::SingleQuoted {
            return Ok(false);
        }
        if backslash_escapes_marker(self.chars, pos, false, None) {
            return Err(invalid_command_error(self.template.to_owned()));
        }
        Ok(true)
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
            ShellQuoteContext::SingleQuoted => QuoteContext::Single('\''),
            ShellQuoteContext::DoubleQuoted => QuoteContext::Double('"'),
        };
        self.output
            .push_str(self.bindings.substitution(placeholder, context));
        Ok(pos + skip)
    }

    /// Return whether the innermost active substitution is arithmetic.
    ///
    /// An arithmetic body evaluates numbers, so `<<` there is a shift
    /// operator. Heredoc tracking must not run inside one: the tracker would
    /// invent a body the shell never starts and end it at a line that is still
    /// inside an expansion.
    fn is_in_arithmetic_expansion(&self) -> bool {
        self.command_substitutions
            .last()
            .is_some_and(delimiters::ScriptCommandSubstitution::is_arithmetic)
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
