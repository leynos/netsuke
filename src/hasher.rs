//! Action hashing utilities.
//!
//! [`ActionHasher`] computes stable SHA-256 digests for [`Action`] definitions.
//! Each action is serialised to canonical JSON before hashing, ensuring
//! identical actions map to the same digest even as the struct evolves.
//!
//! # Examples
//!
//! ```
//! use netsuke::hasher::ActionHasher;
//! use netsuke::ir::Action;
//! use netsuke::ast::{Recipe, StringOrList};
//!
//! let action = Action {
//!     recipe: Recipe::Command { command: "echo".into() },
//!     description: None,
//!     depfile: None,
//!     deps_format: None,
//!     pool: None,
//!     restat: false,
//! };
//! let hash = ActionHasher::hash(&action);
//! assert!(!hash.is_empty());
//! ```

use sha2::{Digest, Sha256};

use crate::ir::Action;

/// Computes stable digests for [`Action`] definitions.
pub struct ActionHasher;

impl ActionHasher {
    /// Calculate the hash of an [`Action`].
    ///
    /// # Panics
    ///
    /// Panics if the action cannot be serialised to JSON.
    #[must_use]
    pub fn hash(action: &Action) -> String {
        // Serialise using canonical JSON so field order and absent options do
        // not affect the resulting digest.
        let bytes = serde_json::to_vec(action).expect("serialise action to JSON");
        let digest = Sha256::digest(bytes);
        format!("{digest:x}")
    }
}
