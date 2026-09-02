//! Best-effort source provenance for the manifest and graph stages.
//!
//! `foreach` expansion rewrites the manifest before typed deserialization, so
//! an expanded item does not always correspond to an authored one. This module
//! recovers the correspondence when it can and returns nothing when it cannot.
//!
//! Guessing is worse than abstaining. A wrong span sends a reader to the wrong
//! line and, because suppression is span-scoped, would let a directive on one
//! target silence a finding about another. Every resolution here is therefore
//! either exact or absent.

use crate::ast::{NetsukeManifest, StringOrList, Target};

use super::document::{Document, Node, Span};

/// Authored nodes matched to the expanded manifest items they produced.
pub struct Provenance<'a> {
    /// Authored node per expanded rule, by declaration index.
    rules: Vec<Option<&'a Node>>,
    /// Authored node per expanded action, by declaration index.
    actions: Vec<Option<&'a Node>>,
    /// Authored node per expanded target, by declaration index.
    targets: Vec<Option<&'a Node>>,
}

impl<'a> Provenance<'a> {
    /// Match every expanded manifest item to its authored node.
    #[must_use]
    pub fn new(doc: &'a Document, manifest: &NetsukeManifest) -> Self {
        Self {
            rules: match_by_name(
                doc.section("rules"),
                manifest.rules.len(),
                &manifest
                    .rules
                    .iter()
                    .map(|rule| Some(rule.name.clone()))
                    .collect::<Vec<_>>(),
            ),
            actions: match_by_name(
                doc.section("actions"),
                manifest.actions.len(),
                &target_names(&manifest.actions),
            ),
            targets: match_by_name(
                doc.section("targets"),
                manifest.targets.len(),
                &target_names(&manifest.targets),
            ),
        }
    }

    /// Borrow the authored node for the rule at `index`.
    #[must_use]
    pub fn rule(&self, index: usize) -> Option<&'a Node> {
        self.rules.get(index).copied().flatten()
    }

    /// Borrow the authored node for the action at `index`.
    #[must_use]
    pub fn action(&self, index: usize) -> Option<&'a Node> {
        self.actions.get(index).copied().flatten()
    }

    /// Borrow the authored node for the target at `index`.
    #[must_use]
    pub fn target(&self, index: usize) -> Option<&'a Node> {
        self.targets.get(index).copied().flatten()
    }
}

/// Report the span of `field` within `node`, preferring the field's key so the
/// diagnostic points at the declaration rather than a long value.
#[must_use]
pub fn field_span(node: Option<&Node>, field: &str) -> Option<Span> {
    let item = node?;
    item.key_node(field)
        .or_else(|| item.get(field))
        .map(|found| found.span)
}

/// Report the span of `node`, when one was resolved.
#[must_use]
pub fn node_span(node: Option<&Node>) -> Option<Span> {
    node.map(|found| found.span)
}

/// Report the span of the entry equal to `value` within `node`'s `field`.
///
/// A dependency list is the common case: the reader wants the offending entry
/// highlighted, not the whole list.
#[must_use]
pub fn entry_span(node: Option<&Node>, field: &str, value: &str) -> Option<Span> {
    let field_node = node?.get(field)?;
    if field_node.as_str() == Some(value) {
        return Some(field_node.span);
    }
    field_node
        .items()
        .find(|item| item.as_str() == Some(value))
        .map(|item| item.span)
        .or(Some(field_node.span))
}

/// Collect the primary name of each target, when it is a single scalar.
fn target_names(targets: &[Target]) -> Vec<Option<String>> {
    targets
        .iter()
        .map(|target| match &target.name {
            StringOrList::String(name) => Some(name.clone()),
            StringOrList::List(names) => names.first().cloned(),
            StringOrList::Empty => None,
        })
        .collect()
}

/// Match expanded items to authored nodes.
///
/// Positional correspondence is used only when the section declares no
/// `foreach` and its authored length equals its expanded length. Equal lengths
/// alone are not enough: a `foreach` over a one-element list leaves the count
/// unchanged while shifting which authored item produced which expanded one,
/// and a wrong span is worse than none.
///
/// Otherwise each expanded item is matched to the sole authored item whose
/// literal `name` scalar equals it; an authored name containing a template is
/// never literal, and an ambiguous match resolves to nothing.
fn match_by_name<'a>(
    section: Option<&'a Node>,
    expanded_len: usize,
    expanded_names: &[Option<String>],
) -> Vec<Option<&'a Node>> {
    let Some(node) = section else {
        return vec![None; expanded_len];
    };
    let authored: Vec<&Node> = node.items().collect();
    let expands = authored.iter().any(|item| item.get("foreach").is_some());
    if !expands && authored.len() == expanded_len {
        return authored.into_iter().map(Some).collect();
    }
    expanded_names
        .iter()
        .map(|candidate| {
            let expected = candidate.as_deref()?;
            let mut matches = authored
                .iter()
                .filter(|item| literal_name(item).is_some_and(|name| name == expected));
            let found = matches.next()?;
            matches.next().is_none().then_some(*found)
        })
        .collect()
}

/// Report an authored item's `name` when it is a literal scalar.
fn literal_name(item: &Node) -> Option<&str> {
    let name = item.get("name")?.as_str()?;
    (!name.contains("{{") && !name.contains("{%")).then_some(name)
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
