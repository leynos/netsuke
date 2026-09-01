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
}

impl CommandSubstitution {
    /// Initialise the state required after a `$(` delimiter.
    pub(super) const fn new() -> Self {
        Self {
            quote_context: QuoteContext::Unquoted,
            parenthesis_depth: 1,
        }
    }
}
