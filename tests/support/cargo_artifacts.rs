//! Parses Cargo compiler-artifact messages for direct-rustc UI harnesses.
//!
//! Cargo 1.99 gives every crate its own artefact directory, so a harness that
//! invokes `rustc` directly must derive its dependency search paths from Cargo
//! JSON rather than assume a shared `target/<profile>/deps` directory.
//!
//! Scope: this module owns parsing and selecting loadable artefact paths from
//! `compiler-artifact` messages. It does not build Cargo packages, deduplicate
//! directories, assemble rustc arguments, or spawn rustc; each harness retains
//! those responsibilities.
//!
//! Reuse policy: include this module only from `tests/*.rs` direct-rustc UI
//! harnesses. Ordinary Cargo-driven tests neither parse these messages nor need
//! Cargo's private artefact layout.

use std::{
    io,
    path::{Path, PathBuf},
};

/// Return loadable artefact parent directories from one Cargo JSON message.
///
/// The directories preserve Cargo's message order. The caller deduplicates
/// across messages because each direct-rustc harness owns its search-path
/// ordering policy.
pub fn dependency_dirs_in_message(line: &str) -> Vec<PathBuf> {
    compiler_artifact_paths(line)
        .map(|(_target, paths)| {
            paths
                .into_iter()
                .filter(|path| is_dependency_artefact(path))
                .filter_map(|path| path.parent().map(Path::to_path_buf))
                .collect()
        })
        .unwrap_or_default()
}

/// Return the preferred metadata path for `target_name` from one Cargo message.
///
/// Rustc type-checks the UI fixtures with `--emit=metadata`. Cargo builds with
/// `-Zembed-metadata=no`, so the matching `.rmeta` is preferred and the rlib
/// remains a compatibility fallback for older Cargo layouts.
pub fn library_path_in_message(line: &str, target_name: &str) -> Option<PathBuf> {
    let (name, paths) = compiler_artifact_paths(line)?;
    (name == target_name)
        .then_some(paths)
        .and_then(|artifact_paths| preferred_library_path(&artifact_paths))
}

/// Select one normal, non-test library artefact from a Cargo JSON stream.
///
/// Compilation units are identified by package ID, target source path, the
/// optional manifest path, and the selected full artefact path. Repeated
/// messages for the same unit are accepted; distinct matching units are
/// rejected rather than allowing output order to choose a host or target
/// artefact.
///
/// # Errors
///
/// Returns an error for malformed Cargo JSON or identity fields, an ambiguous
/// set of matching library units, or a stream without the requested library.
pub fn library_path_in_messages(messages: &str, target_name: &str) -> io::Result<PathBuf> {
    select_library_path(messages, target_name, None)
}

/// Select a normal library artefact belonging to one exact Cargo package ID.
///
/// Other packages with the same target name are ignored before unit selection.
///
/// # Errors
///
/// Returns an error for malformed Cargo JSON or identity fields, an ambiguous
/// set of matching units for `expected_package_id`, or no matching library.
pub fn library_path_in_messages_for_package(
    messages: &str,
    target_name: &str,
    expected_package_id: &str,
) -> io::Result<PathBuf> {
    select_library_path(messages, target_name, Some(expected_package_id))
}

/// Collect matching library units, then apply package and artefact-tier policy.
///
/// # Errors
///
/// Returns an error for malformed Cargo JSON, ambiguous matching units, or a
/// stream without the requested normal library.
fn select_library_path(
    messages: &str,
    target_name: &str,
    expected_package_id: Option<&str>,
) -> io::Result<PathBuf> {
    let mut candidates = Vec::new();
    for (line_index, line) in messages.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let Some(candidate) =
            library_unit_from_message(line, target_name, expected_package_id, line_index + 1)?
        else {
            continue;
        };
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }

    select_candidate_tier(&candidates, expected_package_id.is_none(), target_name)
}

/// Identify one library compilation unit and its selected artefact tier.
#[derive(Debug, PartialEq, Eq)]
struct LibraryArtefactUnit {
    /// Cargo's opaque package identifier.
    package_id: String,
    /// The target's root source file.
    source_path: PathBuf,
    /// The package manifest path when Cargo includes it in the message.
    manifest_path: Option<PathBuf>,
    /// The `.rmeta` path, or `.rlib` fallback, selected for this unit.
    artefact_path: PathBuf,
    /// Whether the message contains metadata without a codegen library file.
    metadata_only: bool,
}

/// Resolve package identity first, then reject ambiguity in the chosen tier.
///
/// # Errors
///
/// Returns an error when unscoped candidates identify different source units,
/// the selected tier contains distinct artefacts, or no candidates exist.
fn select_candidate_tier(
    candidates: &[LibraryArtefactUnit],
    require_one_identity: bool,
    target_name: &str,
) -> io::Result<PathBuf> {
    let identity_conflict = candidates.first().and_then(|first| {
        candidates
            .iter()
            .skip(1)
            .find(|candidate| {
                candidate.package_id != first.package_id
                    || candidate.source_path != first.source_path
                    || candidate.manifest_path != first.manifest_path
            })
            .map(|candidate| (first, candidate))
    });
    if require_one_identity && let Some((first, candidate)) = identity_conflict {
        return Err(io::Error::other(format!(
            "Cargo JSON stream has ambiguous library artefacts for target {target_name:?}: {first:?} and {candidate:?}",
        )));
    }

    let use_metadata_only = candidates.iter().any(|candidate| candidate.metadata_only);
    let mut selected: Option<&LibraryArtefactUnit> = None;
    for candidate in candidates
        .iter()
        .filter(|candidate| candidate.metadata_only == use_metadata_only)
    {
        if let Some(previous) = selected {
            if previous != candidate {
                return Err(io::Error::other(format!(
                    "Cargo JSON stream has ambiguous library artefacts for target {target_name:?}: {previous:?} and {candidate:?}",
                )));
            }
        } else {
            selected = Some(candidate);
        }
    }

    selected
        .map(|candidate| candidate.artefact_path.clone())
        .ok_or_else(|| {
            io::Error::other(format!(
                "Cargo JSON stream has no normal, non-test library artefact for target {target_name:?}",
            ))
        })
}

/// Return a required non-empty JSON string or describe its malformed field.
///
/// # Errors
///
/// Returns an error when the field is absent, empty, or not a string.
fn required_string(
    value: Option<&serde_json::Value>,
    field: &str,
    line: usize,
) -> io::Result<String> {
    value
        .and_then(serde_json::Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| malformed_field(line, field))
}

/// Return whether a required JSON string array contains `expected`.
///
/// # Errors
///
/// Returns an error when the field is absent, is not an array, or contains a
/// non-string value.
fn string_array_contains(
    value: Option<&serde_json::Value>,
    field: &str,
    expected: &str,
    line: usize,
) -> io::Result<bool> {
    let values = value
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| malformed_field(line, field))?;
    let mut contains_expected = false;
    for item in values {
        let text = item.as_str().ok_or_else(|| malformed_field(line, field))?;
        contains_expected |= text == expected;
    }
    Ok(contains_expected)
}

/// Parse Cargo's optional manifest path while rejecting malformed values.
///
/// # Errors
///
/// Returns an error when the field is present but empty or not a string.
fn optional_manifest_path(message: &serde_json::Value, line: usize) -> io::Result<Option<PathBuf>> {
    message
        .get("manifest_path")
        .map(|path| required_string(Some(path), "manifest_path", line).map(PathBuf::from))
        .transpose()
}

/// Describe a missing or incorrectly typed field in a Cargo JSON message.
fn malformed_field(line: usize, field: &str) -> io::Error {
    io::Error::other(format!(
        "Cargo compiler-artifact JSON line {line} has a missing or invalid {field}",
    ))
}

/// Return the target name and filenames in one compiler-artifact message.
fn compiler_artifact_paths(line: &str) -> Option<(String, Vec<PathBuf>)> {
    let message: serde_json::Value = serde_json::from_str(line).ok()?;
    if message.get("reason")?.as_str()? != "compiler-artifact" {
        return None;
    }
    let name = message.get("target")?.get("name")?.as_str()?.to_owned();
    let paths = message
        .get("filenames")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(PathBuf::from)
        .collect();
    Some((name, paths))
}

/// Return whether `path` names an artefact rustc can load through `-L`.
fn is_dependency_artefact(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("rmeta")
            || extension.eq_ignore_ascii_case("rlib")
            || extension.eq_ignore_ascii_case(std::env::consts::DLL_EXTENSION)
    })
}

/// Parse the identity and artefact of one matching normal library unit.
///
/// # Errors
///
/// Returns an error when Cargo JSON is malformed, a matching message has
/// invalid identity fields, or it does not report a loadable library artefact.
fn library_unit_from_message(
    line: &str,
    target_name: &str,
    expected_package_id: Option<&str>,
    line_number: usize,
) -> io::Result<Option<LibraryArtefactUnit>> {
    let message: serde_json::Value = serde_json::from_str(line).map_err(|error| {
        io::Error::other(format!(
            "Cargo JSON line {line_number} could not be parsed: {error}",
        ))
    })?;
    let reason = required_string(message.get("reason"), "reason", line_number)?;
    if reason != "compiler-artifact" {
        return Ok(None);
    }

    let target = message
        .get("target")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| malformed_field(line_number, "target object"))?;
    let name = required_string(target.get("name"), "target.name", line_number)?;
    if name != target_name {
        return Ok(None);
    }

    let package_id = required_string(message.get("package_id"), "package_id", line_number)?;
    if expected_package_id.is_some_and(|expected| expected != package_id.as_str()) {
        return Ok(None);
    }
    let source_path = PathBuf::from(required_string(
        target.get("src_path"),
        "target.src_path",
        line_number,
    )?);
    let manifest_path = optional_manifest_path(&message, line_number)?;
    if !is_normal_library_unit(&message, target, line_number)? {
        return Ok(None);
    }

    let (artefact_path, metadata_only) = library_artefact_path(line, target_name, line_number)?;
    Ok(Some(LibraryArtefactUnit {
        package_id,
        source_path,
        manifest_path,
        artefact_path,
        metadata_only,
    }))
}

/// Check whether a target is a non-test Rust library compilation unit.
///
/// # Errors
///
/// Returns an error when Cargo omits or malforms the target kind, crate types,
/// or test-profile indicator.
fn is_normal_library_unit(
    message: &serde_json::Value,
    target: &serde_json::Map<String, serde_json::Value>,
    line_number: usize,
) -> io::Result<bool> {
    let is_library_kind =
        string_array_contains(target.get("kind"), "target.kind", "lib", line_number)?;
    let is_library_crate_type = string_array_contains(
        target.get("crate_types"),
        "target.crate_types",
        "lib",
        line_number,
    )?;
    let is_test = message
        .get("profile")
        .and_then(serde_json::Value::as_object)
        .and_then(|profile| profile.get("test"))
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| malformed_field(line_number, "profile.test boolean"))?;
    Ok(!is_test && is_library_kind && is_library_crate_type)
}

/// Select the metadata path, or the library fallback, from one Cargo message.
///
/// # Errors
///
/// Returns an error when Cargo does not provide a loadable library path.
fn library_artefact_path(
    line: &str,
    target_name: &str,
    line_number: usize,
) -> io::Result<(PathBuf, bool)> {
    let (_, paths) = compiler_artifact_paths(line)
        .ok_or_else(|| malformed_field(line_number, "compiler-artifact filenames array"))?;
    let artefact_path = preferred_library_path(&paths).ok_or_else(|| {
        io::Error::other(format!(
            "Cargo JSON line {line_number} has no loadable library artefact for target {target_name:?}",
        ))
    })?;
    let metadata_only = last_with_extension(&paths, "rmeta").is_some()
        && last_with_extension(&paths, "rlib").is_none();
    Ok((artefact_path, metadata_only))
}

/// Prefer the last metadata path and fall back to the last library path.
fn preferred_library_path(paths: &[PathBuf]) -> Option<PathBuf> {
    last_with_extension(paths, "rmeta").or_else(|| last_with_extension(paths, "rlib"))
}

/// Return the last path with `extension`, retaining Cargo's uplift ordering.
fn last_with_extension(paths: &[PathBuf], extension: &str) -> Option<PathBuf> {
    paths
        .iter()
        .rfind(|path| {
            path.extension()
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(extension))
        })
        .cloned()
}

#[cfg(test)]
#[path = "cargo_artifact_parser_tests.rs"]
mod parser_tests;

#[cfg(test)]
#[path = "cargo_artifact_selection_tests.rs"]
mod selection_tests;
