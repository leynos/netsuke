//! Model the required non-interactive execution setting for CLI configuration.

use serde::{Deserialize, Serialize};

/// Required non-interactive execution setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NoInput(bool);

impl NoInput {
    /// Return whether interactive input is disabled.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        self.0
    }
}

impl Default for NoInput {
    fn default() -> Self {
        Self(true)
    }
}
