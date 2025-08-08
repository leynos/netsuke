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
//! let hash = ActionHasher::hash(&action).expect("failed to hash action");
//! assert!(!hash.is_empty());
//! ```

use sha2::{Digest, Sha256};

use crate::ir::Action;
use serde_json_canonicalizer::to_writer;
use std::io::{self, Write};

/// Computes stable digests for [`Action`] definitions.
pub struct ActionHasher;

struct DigestWriter<'a, D: Digest>(&'a mut D);

impl<D: Digest> Write for DigestWriter<'_, D> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl ActionHasher {
    /// Calculate the hash of an [`Action`].
    ///
    /// Returns a lowercase hex-encoded SHA-256 of the action's canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if the action cannot be serialised to JSON.
    pub fn hash(action: &Action) -> Result<String, serde_json::Error> {
        let mut hasher = Sha256::new();
        {
            // Canonical JSON: compact formatting with sorted keys.
            let mut writer = DigestWriter(&mut hasher);
            to_writer(action, &mut writer)?;
        }
        Ok(format!("{:x}", hasher.finalize()))
    }
}
