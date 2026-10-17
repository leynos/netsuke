//! Resolves root-package build dependency IDs from Cargo metadata.
//!
//! Scope: this module runs the locked, all-features metadata query and selects
//! direct build-dependency package IDs for the production build-slice harness.
//! It does not select compiler artefacts or assemble rustc arguments.
//!
//! Reuse policy: this helper is owned by the build-module-slice UI harness;
//! other harnesses should use a narrowly scoped resolver if they need this data.

use std::{collections::HashSet, io, path::Path, process::Command};

/// Query Cargo for the exact package IDs of the requested root build dependencies.
///
/// # Errors
///
/// Returns an error when Cargo metadata fails or its dependency graph is
/// malformed, ambiguous, or missing a requested build dependency.
pub fn build_dependency_package_ids(
    cargo_executable: &Path,
    manifest_path: &Path,
    required_names: &[&'static str],
) -> io::Result<Vec<(&'static str, String)>> {
    let output = Command::new(cargo_executable)
        .args([
            "metadata",
            "--format-version=1",
            "--all-features",
            "--locked",
            "--manifest-path",
        ])
        .arg(manifest_path)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "cargo metadata failed ({}):\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        )));
    }

    build_dependency_ids_from_metadata(&String::from_utf8_lossy(&output.stdout), required_names)
}

/// Parse requested root build-dependency package IDs from one metadata document.
///
/// # Errors
///
/// Returns an error when Cargo metadata is malformed or a requested name does
/// not resolve to exactly one unique package ID.
fn build_dependency_ids_from_metadata(
    metadata_json: &str,
    required_names: &[&'static str],
) -> io::Result<Vec<(&'static str, String)>> {
    let metadata: serde_json::Value = serde_json::from_str(metadata_json).map_err(|error| {
        io::Error::other(format!("cargo metadata output is not valid JSON: {error}"))
    })?;
    let (root_node, node_ids) = resolve_root_node(&metadata)?;
    let dependencies = root_node
        .get("deps")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| invalid_metadata("root node deps array"))?;
    let mut selected_ids = required_names
        .iter()
        .map(|&name| (name, Vec::new()))
        .collect::<Vec<(&'static str, Vec<String>)>>();
    collect_build_edges(dependencies, &node_ids, &mut selected_ids)?;
    resolve_required_ids(selected_ids)
}

/// Find the unique root node and validate every package node identifier.
///
/// # Errors
///
/// Returns an error when the resolve root, node list, or node identities are
/// missing, malformed, or duplicated.
fn resolve_root_node(
    metadata: &serde_json::Value,
) -> io::Result<(&serde_json::Value, HashSet<String>)> {
    let resolve = metadata
        .get("resolve")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| invalid_metadata("resolve object"))?;
    let root_id = required_metadata_string(resolve.get("root"), "resolve.root")?;
    let nodes = resolve
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| invalid_metadata("resolve.nodes array"))?;
    let mut node_ids = HashSet::with_capacity(nodes.len());
    let mut matching_root_node = None;
    for (index, node) in nodes.iter().enumerate() {
        let node_object = node
            .as_object()
            .ok_or_else(|| invalid_metadata(&format!("resolve.nodes[{index}] object")))?;
        let id = required_metadata_string(node_object.get("id"), "resolve.nodes[].id")?;
        if !node_ids.insert(id.to_owned()) {
            return Err(invalid_metadata(&format!(
                "duplicate resolve node ID {id:?}",
            )));
        }
        if id == root_id {
            matching_root_node = Some(node);
        }
    }

    let root_node = matching_root_node.ok_or_else(|| invalid_metadata("resolve root node"))?;
    Ok((root_node, node_ids))
}

/// Collect package IDs for matching direct build-dependency edges.
///
/// # Errors
///
/// Returns an error when a dependency edge is malformed or references a
/// package absent from the resolved node list.
fn collect_build_edges(
    dependencies: &[serde_json::Value],
    node_ids: &HashSet<String>,
    selected_ids: &mut [(&'static str, Vec<String>)],
) -> io::Result<()> {
    for (index, dependency) in dependencies.iter().enumerate() {
        let edge = dependency
            .as_object()
            .ok_or_else(|| invalid_metadata(&format!("root deps[{index}] object")))?;
        let name = required_metadata_string(edge.get("name"), "root deps[].name")?;
        let package_id = required_metadata_string(edge.get("pkg"), "root deps[].pkg")?;
        if !node_ids.contains(package_id) {
            return Err(invalid_metadata(&format!(
                "root dependency {name:?} references unknown package ID {package_id:?}",
            )));
        }
        if !has_build_dependency_kind(edge, index)? {
            continue;
        }

        if let Some((_, package_ids)) = selected_ids
            .iter_mut()
            .find(|(required_name, _)| *required_name == name)
            && !package_ids.iter().any(|existing| existing == package_id)
        {
            package_ids.push(package_id.to_owned());
        }
    }
    Ok(())
}

/// Check whether a root dependency edge includes the build dependency kind.
///
/// # Errors
///
/// Returns an error when the edge omits or malforms its dependency-kind list.
fn has_build_dependency_kind(
    edge: &serde_json::Map<String, serde_json::Value>,
    edge_index: usize,
) -> io::Result<bool> {
    let kinds = edge
        .get("dep_kinds")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| invalid_metadata(&format!("root deps[{edge_index}].dep_kinds array")))?;
    let mut has_build_kind = false;
    for (index, dependency_kind) in kinds.iter().enumerate() {
        let kind_object = dependency_kind.as_object().ok_or_else(|| {
            invalid_metadata(&format!(
                "root deps[{edge_index}].dep_kinds[{index}] object"
            ))
        })?;
        let kind = kind_object.get("kind").ok_or_else(|| {
            invalid_metadata(&format!("root deps[{edge_index}].dep_kinds[{index}].kind"))
        })?;
        match kind {
            serde_json::Value::Null => {}
            serde_json::Value::String(value) if !value.is_empty() => {
                has_build_kind |= value == "build";
            }
            _ => {
                return Err(invalid_metadata(&format!(
                    "root deps[{edge_index}].dep_kinds[{index}].kind",
                )));
            }
        }
    }
    Ok(has_build_kind)
}

/// Require each requested name to resolve to one distinct package ID.
///
/// # Errors
///
/// Returns an error when a requested name is missing or has multiple package IDs.
fn resolve_required_ids(
    selected_ids: Vec<(&'static str, Vec<String>)>,
) -> io::Result<Vec<(&'static str, String)>> {
    selected_ids
        .into_iter()
        .map(|(name, package_ids)| match package_ids.as_slice() {
            [package_id] => Ok((name, package_id.clone())),
            [] => Err(io::Error::other(format!(
                "cargo metadata has no build dependency named {name:?}",
            ))),
            _ => Err(io::Error::other(format!(
                "cargo metadata has ambiguous build dependency {name:?}: {package_ids:?}",
            ))),
        })
        .collect()
}

/// Return a required non-empty Cargo metadata string.
///
/// # Errors
///
/// Returns an error when the field is absent, empty, or not a string.
fn required_metadata_string<'a>(
    value: Option<&'a serde_json::Value>,
    field: &str,
) -> io::Result<&'a str> {
    value
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| invalid_metadata(field))
}

/// Describe a missing or malformed Cargo metadata field.
fn invalid_metadata(field: &str) -> io::Error {
    io::Error::other(format!("cargo metadata has a missing or invalid {field}"))
}

#[cfg(test)]
#[path = "cargo_build_dependency_ids_tests.rs"]
mod tests;
