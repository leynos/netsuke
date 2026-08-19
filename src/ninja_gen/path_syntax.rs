//! Ninja path validation and escaping.
//!
//! This module owns the characters that Ninja can represent in build and
//! dyndep paths. Callers validate whole graphs before emitting output, while
//! individual dyndep renderers retain a fallible escaping boundary.

use super::NinjaGenError;
use crate::ir::BuildGraph;
use crate::localization::{self, keys};
use camino::Utf8PathBuf;

/// Escape a Ninja path for embedding in a build or dyndep document.
pub(crate) fn escape_ninja_path(path: &str) -> Result<String, NinjaGenError> {
    validate_path(path)?;
    Ok(escape_validated_ninja_path(path))
}

/// Escape a path after whole-graph syntax validation has succeeded.
pub(super) fn escape_validated_ninja_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for ch in path.chars() {
        match ch {
            ' ' => out.push_str("$ "),
            '$' => out.push_str("$$"),
            ':' => out.push_str("$:"),
            _ => out.push(ch),
        }
    }
    out
}

/// Reject graph paths that Ninja cannot represent.
pub(crate) fn reject_unsupported_path_characters(graph: &BuildGraph) -> Result<(), NinjaGenError> {
    for edge in graph.targets.values() {
        for path in edge
            .explicit_outputs
            .iter()
            .chain(&edge.implicit_outputs)
            .chain(&edge.inputs)
            .chain(&edge.implicit_deps)
            .chain(&edge.order_only_deps)
        {
            validate_path(path.as_str())?;
        }
    }
    for path in &graph.default_targets {
        validate_path(path.as_str())?;
    }
    Ok(())
}

/// Ensure `path` contains no character Ninja cannot represent.
fn validate_path(path: &str) -> Result<(), NinjaGenError> {
    if let Some(character) = unsupported_character(path) {
        return Err(unsupported_path_character(path, character));
    }
    Ok(())
}

/// Return the first Ninja-unsupported character in `path`, if any.
fn unsupported_character(path: &str) -> Option<char> {
    path.chars()
        .find(|character| matches!(character, '|' | '\t' | '\r' | '\n'))
}

/// Build the error naming `path` and the single unsupported `character`.
fn unsupported_path_character(path: &str, character: char) -> NinjaGenError {
    NinjaGenError::UnsupportedPathCharacter {
        path: Utf8PathBuf::from(path),
        character,
        message: localization::message(keys::NINJA_GEN_UNSUPPORTED_PATH_CHARACTER)
            .with_arg("path", path)
            .with_arg("character", character),
    }
}
