//! Action hashing utilities.
//!
//! This module provides the [`ActionHasher`] type used to compute a stable
//! SHA-256 digest for [`Action`] definitions. The hash is used to deduplicate
//! identical actions when generating the build graph.
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

use crate::ast::{Recipe, StringOrList};
use crate::ir::Action;

/// Computes stable digests for [`Action`] definitions.
pub struct ActionHasher;

impl ActionHasher {
    /// Calculate the hash of an [`Action`].
    #[must_use]
    pub fn hash(action: &Action) -> String {
        let mut hasher = Sha256::new();
        Self::hash_recipe(&mut hasher, &action.recipe);
        Self::hash_optional_fields(&mut hasher, action);
        format!("{:x}", hasher.finalize())
    }

    fn hash_recipe(hasher: &mut Sha256, recipe: &Recipe) {
        match recipe {
            Recipe::Command { command } => {
                hasher.update(b"cmd");
                Self::update_with_len(hasher, command.as_bytes());
            }
            Recipe::Script { script } => {
                hasher.update(b"scr");
                Self::update_with_len(hasher, script.as_bytes());
            }
            Recipe::Rule { rule } => {
                hasher.update(b"rule");
                Self::hash_rule_reference(hasher, rule);
            }
        }
    }

    fn hash_optional_fields(hasher: &mut Sha256, action: &Action) {
        Self::hash_optional_string(hasher, action.description.as_ref());
        Self::hash_optional_string(hasher, action.depfile.as_ref());
        Self::hash_optional_string(hasher, action.deps_format.as_ref());
        Self::hash_optional_string(hasher, action.pool.as_ref());
        hasher.update(if action.restat { b"1" } else { b"0" });
    }

    fn hash_rule_reference(hasher: &mut Sha256, rule: &StringOrList) {
        match rule {
            StringOrList::String(r) => Self::update_with_len(hasher, r.as_bytes()),
            StringOrList::List(list) => {
                for r in list {
                    Self::update_with_len(hasher, r.as_bytes());
                }
            }
            StringOrList::Empty => {}
        }
    }

    fn hash_optional_string(hasher: &mut Sha256, value: Option<&String>) {
        match value {
            Some(v) => {
                hasher.update(b"1");
                Self::update_with_len(hasher, v.as_bytes());
            }
            None => hasher.update(b"0"),
        }
    }

    fn update_with_len(hasher: &mut Sha256, bytes: &[u8]) {
        let len = bytes.len();
        hasher.update(format!("{len}:").as_bytes());
        hasher.update(bytes);
    }
}
