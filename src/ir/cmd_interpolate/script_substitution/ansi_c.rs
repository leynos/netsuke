//! Open and close ANSI-C strings inside one POSIX or Bash script.
//!
//! The parent traversal keeps its own quote-context stack; this child owns the
//! two transitions that only an ANSI-C string has, so the traversal stays
//! within the repository's module-size cap.

use super::super::posix_lexical::AnsiCCharacter;
use super::{ScriptSubstitutionTraversal, ShellQuoteContext};

impl ScriptSubstitutionTraversal<'_, '_> {
    /// Open or close an ANSI-C string, reporting whether it decided the character.
    ///
    /// A dollar followed by a quote opens the region before the ordinary quote
    /// rules run, and inside it only an unescaped apostrophe closes. `None`
    /// means neither happened, so the caller applies the ordinary rules.
    pub(super) fn update_ansi_c_context(&mut self, pos: usize, ch: char) -> Option<bool> {
        let opens = self.posix_lexical.observe_ansi_c_character(
            AnsiCCharacter::new(self.chars, pos, ch),
            self.active_quote_context() == ShellQuoteContext::Unquoted,
        );
        let quote_context = self.active_quote_context_mut();
        if opens {
            *quote_context = ShellQuoteContext::AnsiC;
            return Some(false);
        }
        if *quote_context != ShellQuoteContext::AnsiC {
            return None;
        }
        if ch != '\'' {
            return Some(false);
        }
        *quote_context = ShellQuoteContext::Unquoted;
        Some(true)
    }
}
