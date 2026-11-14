//! Answer and update quoting for the innermost active shell region.
//!
//! A command substitution carries its own quote context, so every question about
//! quoting has to name the region it applies to: the innermost open substitution
//! when there is one, and the traversal's own context otherwise. Keeping that
//! choice in one place stops a caller from reading the outer context while the
//! shell is inside an inner region, which would misclassify a number sign as a
//! comment or a marker as protected.

use super::{QuoteContext, SubstitutionTraversal};

impl SubstitutionTraversal<'_, '_> {
    /// Change POSIX quote state after retaining a delimiter in the output.
    pub(super) fn update_quote_context(&mut self, ch: char) {
        let quote_context = self.active_quote_context_mut();
        Self::update_quote_context_for_region(quote_context, ch);
    }

    /// Report whether the traversal is currently enclosed by single quotes.
    pub(super) fn matches_single_quote_context(&self) -> bool {
        matches!(self.active_quote_context(), QuoteContext::Single(_))
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
