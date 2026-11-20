//! Hold the state required for one-pass recipe placeholder substitution.
//!
//! Command-substitution delimiters and the `case` boundaries they imply live
//! in [`delimiters`], so this module owns the traversal state itself: the
//! character-by-character dispatch, shell-specific delegation, and marker
//! lowering for the active quote context.

use super::{
    CommandBindings, IrGenError, QuoteContext, command_substitution::CommandSubstitution,
    find_substitution, invalid_command_error, posix_lexical::PosixLexicalState,
};
use crate::recipe_shell::RecipeShell;

mod delimiters;

mod posix;

mod powershell;

mod quotes;

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

    /// Open or close a backtick command-substitution region.
    ///
    /// Backticks are the legacy spelling of `$( )`, and bash parses their body
    /// as its own command region. Scoping the `${ }` count the same way keeps a
    /// brace inside the body from closing an expansion outside it, which would
    /// otherwise read the following number sign as a comment and hide a heredoc
    /// declaration.
    fn toggle_backtick_region(&mut self) {
        if self.in_backticks {
            self.posix_lexical.leave_command_substitution_region();
        } else {
            self.posix_lexical.enter_command_substitution_region();
        }
        self.in_backticks = !self.in_backticks;
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
