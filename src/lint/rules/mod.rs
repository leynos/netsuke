//! The shipped lint rules, grouped by the concern they address.
//!
//! Rules are grouped by category rather than one file per rule so that a
//! category's shared helpers stay next to the rules that use them, following
//! the repository's group-by-feature convention. Every rule is registered
//! exactly once by [`registered`].

use super::registry::Registered;

pub mod caching;
pub mod clarity;
pub mod determinism;
pub mod graph;
pub mod hygiene;
pub mod migration;
pub mod portability;
pub mod recipes;
pub mod redundancy;
pub mod shellscan;
pub mod suppression;

/// Every rule the linter ships.
#[must_use]
pub fn registered() -> Vec<Registered> {
    let mut rules = Vec::new();
    rules.extend(migration::rules());
    rules.extend(portability::rules());
    rules.extend(determinism::rules());
    rules.extend(clarity::rules());
    rules.extend(redundancy::rules());
    rules.extend(hygiene::rules());
    rules.extend(caching::rules());
    rules.extend(graph::rules());
    rules.extend(suppression::rules());
    rules
}
