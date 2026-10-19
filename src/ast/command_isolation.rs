//! Defines the state-sharing policy for ordered command-list recipes.

use serde::{Deserialize, Serialize};

/// Select whether a command-list entry shares shell state with later entries.
///
/// ```
/// use netsuke::ast::CommandIsolation;
///
/// let default_mode = CommandIsolation::default();
/// assert_eq!(default_mode, CommandIsolation::Shared);
/// assert_ne!(CommandIsolation::Isolated, default_mode);
/// ```
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandIsolation {
    /// Run entries in one shell so state changes reach later entries.
    #[default]
    Shared,
    /// Start each entry from the action's working directory and environment.
    Isolated,
}

impl CommandIsolation {
    /// Report whether this mode preserves the legacy shared-state behaviour.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::ast::CommandIsolation;
    ///
    /// assert_eq!(CommandIsolation::Shared, CommandIsolation::default());
    /// assert_ne!(CommandIsolation::Isolated, CommandIsolation::default());
    /// ```
    #[must_use]
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "Serde skip_serializing_if predicates receive a borrowed field"
    )]
    pub(crate) const fn is_shared(&self) -> bool {
        matches!(self, Self::Shared)
    }
}
