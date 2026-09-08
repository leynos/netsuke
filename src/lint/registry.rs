//! The static rule registry.
//!
//! Every rule the linter ships is listed here exactly once. The registry is
//! the source of truth for rule identity, category, default severity, and rule
//! text, and a contract test checks the rule reference document against it, so
//! the shipped rules and their documentation cannot drift apart.

use super::rule::{DirectiveRule, DocumentRule, GraphRule, ManifestRule, RuleMeta};
use super::rules;

/// One registered rule, tagged by the stage it binds to.
pub enum Registered {
    /// A rule inspecting the authored source.
    Document(&'static dyn DocumentRule),
    /// A rule inspecting the expanded and rendered manifest.
    Manifest(&'static dyn ManifestRule),
    /// A rule inspecting the lowered build graph.
    Graph(&'static dyn GraphRule),
    /// A rule inspecting the lint directives themselves.
    Directive(&'static dyn DirectiveRule),
}

impl Registered {
    /// Describe the registered rule.
    #[must_use]
    pub fn meta(&self) -> &'static RuleMeta {
        match self {
            Self::Document(rule) => rule.meta(),
            Self::Manifest(rule) => rule.meta(),
            Self::Graph(rule) => rule.meta(),
            Self::Directive(rule) => rule.meta(),
        }
    }
}

/// Every rule the linter ships, in registration order.
///
/// Registration order does not affect output: findings are sorted by position
/// before they are reported.
#[must_use]
pub fn all() -> Vec<Registered> {
    rules::registered()
}

/// Describe every registered rule.
pub fn all_meta() -> impl Iterator<Item = &'static RuleMeta> {
    all().into_iter().map(|entry| entry.meta())
}

/// Look up a rule's metadata by its stable name.
#[must_use]
pub fn meta_by_name(name: &str) -> Option<&'static RuleMeta> {
    all_meta().find(|meta| meta.name == name)
}

/// Report whether `name` identifies a registered rule.
#[must_use]
pub fn is_known(name: &str) -> bool {
    meta_by_name(name).is_some()
}

/// Describe every registered rule, ordered by category then name.
///
/// This is the order `--explain` and the rule reference present, which groups
/// rules by the concern they address rather than by the stage they happen to
/// bind to.
#[must_use]
pub fn catalogue() -> Vec<&'static RuleMeta> {
    let mut entries: Vec<&'static RuleMeta> = all_meta().collect();
    entries.sort_by_key(|meta| (meta.category, meta.name));
    entries
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
