//! Answer and update quoting for the innermost active shell region.
//!
//! A command substitution carries its own quote context, so every question about
//! quoting has to name the region it applies to: the innermost open substitution
//! when there is one, and the traversal's own context otherwise. Keeping that
//! choice in one place stops a caller from reading the outer context while the
//! shell is inside an inner region, which would misclassify a number sign as a
//! comment or a marker as protected.

use super::{QuoteContext, SubstitutionTraversal};
use crate::ir::cmd_interpolate::posix_lexical::AnsiCCharacter;
use crate::recipe_shell::RecipeShell;

impl SubstitutionTraversal<'_, '_> {
    /// Change POSIX quote state after retaining a delimiter in the output.
    pub(super) fn update_quote_context(&mut self, pos: usize, ch: char) {
        // ANSI-C quoting is a POSIX and Bash rule. PowerShell shares this
        // traversal but reads `$'` differently, so its model is left alone.
        let opens = self.bindings.shell != RecipeShell::PowerShell
            && self.posix_lexical.observe_ansi_c_character(
                AnsiCCharacter::new(self.chars, pos, ch),
                self.matches_unquoted_quote_context(),
            );
        let quote_context = self.active_quote_context_mut();
        if opens {
            *quote_context = QuoteContext::AnsiC;
            return;
        }
        // Inside ANSI-C only an unescaped apostrophe closes the string; a
        // backslash-escaped one was already consumed as a pair, so anything
        // reaching here is unescaped.
        if matches!(*quote_context, QuoteContext::AnsiC) && ch == '\'' {
            *quote_context = QuoteContext::Unquoted;
            return;
        }
        Self::update_quote_context_for_region(quote_context, ch);
    }

    /// Report whether `$(` and `)` are substitution syntax in this context.
    ///
    /// Bash expands neither inside ordinary single quotes nor inside an
    /// ANSI-C string, where both are literal text.
    pub(super) fn matches_substitution_context(&self) -> bool {
        !matches!(
            self.active_quote_context(),
            QuoteContext::Single(_) | QuoteContext::AnsiC
        )
    }

    /// Report whether the traversal is currently enclosed by single quotes.
    pub(super) fn matches_single_quote_context(&self) -> bool {
        matches!(self.active_quote_context(), QuoteContext::Single(_))
    }

    /// Report whether the traversal is currently inside a Bash ANSI-C string.
    ///
    /// Backticks are literal there, so a backtick must not open or close a
    /// legacy command region while this is true.
    pub(super) fn matches_ansi_c_context(&self) -> bool {
        matches!(self.active_quote_context(), QuoteContext::AnsiC)
    }

    /// Report whether the active shell region is outside both kinds of quotes.
    pub(super) fn matches_unquoted_quote_context(&self) -> bool {
        matches!(self.active_quote_context(), QuoteContext::Unquoted)
    }

    /// Return the quote state used by the innermost active shell region.
    pub(super) fn active_quote_context(&self) -> QuoteContext {
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
}
