//! Ninja path validation.
//!
//! This module owns the path characters that Netsuke permits in build and
//! dyndep documents. Callers validate whole graphs before emitting output,
//! while individual dyndep renderers retain the same fallible boundary.

use super::NinjaGenError;
use crate::ir::BuildGraph;
use crate::localization::{self, keys};
use camino::Utf8PathBuf;

/// Validate and clone a Ninja path for embedding in a build or dyndep document.
pub(crate) fn validated_ninja_path(path: &str) -> Result<String, NinjaGenError> {
    validate_path(path)?;
    Ok(clone_validated_ninja_path(path))
}

/// Clone a path after whole-graph syntax validation has succeeded.
pub(super) fn clone_validated_ninja_path(path: &str) -> String {
    path.to_owned()
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

/// Ensure `path` contains no character Netsuke can emit without ambiguity.
fn validate_path(path: &str) -> Result<(), NinjaGenError> {
    if let Some(character) = unsupported_character(path) {
        return Err(unsupported_path_character(path, character));
    }
    Ok(())
}

/// Return the first unsupported character in `path`, if any.
fn unsupported_character(path: &str) -> Option<char> {
    path.chars()
        .find(|character| matches!(character, '$' | ' ' | ':' | '|') || character.is_control())
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
