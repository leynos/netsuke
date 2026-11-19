//! XML lookups shared by the installer package contracts.
//!
//! The `WiX` authoring and the frozen v0.1.0-beta4 fixture are both read with
//! `roxmltree`, and every contract locates its subject the same way: by
//! descendant tag and attribute, by child tag, or by comparing an element's
//! attributes. These helpers live in a directory module so the test binary
//! stays inside the repository's file-size cap without duplicating the
//! lookups across its two fixtures.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use roxmltree::{Document, Node};

/// Find the first descendant element with the given tag name and attribute.
pub fn find_element<'a, 'input>(
    document: &'a Document<'input>,
    tag_name: &str,
    attribute: &str,
    value: &str,
) -> Result<Node<'a, 'input>> {
    document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == tag_name
                && node.attribute(attribute) == Some(value)
        })
        .with_context(|| format!("expected {tag_name} with {attribute}={value}"))
}

/// Find the first child element with the given tag name.
pub fn find_child<'a, 'input>(
    parent: Node<'a, 'input>,
    tag_name: &str,
) -> Result<Node<'a, 'input>> {
    parent
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == tag_name)
        .with_context(|| format!("expected a {tag_name} child"))
}

/// Read an element's attributes as a sorted map.
pub fn comparable_attributes(node: Node<'_, '_>) -> BTreeMap<String, String> {
    node.attributes()
        .map(|attribute| (attribute.name().to_owned(), attribute.value().to_owned()))
        .collect()
}

/// Return the registry search that reads the installed release rank.
pub fn rank_search<'a, 'input>(document: &'a Document<'input>) -> Result<Node<'a, 'input>> {
    let property = find_element(document, "Property", "Id", "PREVIOUS_NETSUKE_RELEASE_RANK")?;
    find_child(property, "RegistrySearch")
}
