//! Retain local state for one POSIX-style command substitution.
//!
//! Command substitutions have independent quote and parenthesis state, which
//! prevents their contents from changing the context of surrounding recipe
//! markers.

use super::QuoteContext;

/// Identify one command-substitution delimiter recognised during traversal.
pub(super) enum CommandSubstitutionDelimiter {
    /// Begin a nested `$()` command substitution.
    Start,
    /// Open a grouped expression within the active command substitution.
    NestedOpen,
    /// Close a grouped expression or the active command substitution.
    Close,
}

/// Retain parsing state for one active command substitution.
pub(super) struct CommandSubstitution {
    /// Record the quote state local to this command-substitution body.
    pub(super) quote_context: QuoteContext,
    /// Count this substitution's opening parenthesis and nested grouped forms.
    pub(super) parenthesis_depth: usize,
    /// Record whether the body is a `$(( ))` arithmetic expansion.
    pub(super) is_arithmetic_expansion: bool,
}

impl CommandSubstitution {
    /// Initialise the state required after a `$(` delimiter.
    ///
    /// `is_arithmetic_expansion` records whether the delimiter was `$((`, so a
    /// caller can keep arithmetic operators such as `<<` away from shell
    /// heredoc tracking. Arithmetic bodies expand numbers, so they never hold
    /// a heredoc declaration.
    pub(super) const fn new(is_arithmetic_expansion: bool) -> Self {
        Self {
            quote_context: QuoteContext::Unquoted,
            parenthesis_depth: 1,
            is_arithmetic_expansion,
        }
    }
}
