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

use std::path::{Path, PathBuf};

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
        .and_then(|artifact_paths| {
            last_with_extension(&artifact_paths, "rmeta")
                .or_else(|| last_with_extension(&artifact_paths, "rlib"))
        })
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
mod tests {
    //! Property and example tests for the Cargo artefact parser.

    use super::{dependency_dirs_in_message, library_path_in_message};
    use proptest::prelude::*;
    use std::path::{Path, PathBuf};

    /// Generate a newline-free Cargo artefact path and whether rustc can load it.
    fn artefact_path() -> impl Strategy<Value = (String, bool)> {
        (
            "[\\p{L}\\p{N} _-]{1,12}",
            "[\\p{L}\\p{N}_-]{1,12}",
            prop_oneof![
                Just("rmeta"),
                Just("rlib"),
                Just(std::env::consts::DLL_EXTENSION),
                Just("txt")
            ],
        )
            .prop_map(|(directory, name, extension)| {
                let path = format!("/tmp/{directory}/lib{name}.{extension}");
                let loadable = matches!(extension, "rmeta" | "rlib")
                    || extension.eq_ignore_ascii_case(std::env::consts::DLL_EXTENSION);
                (path, loadable)
            })
    }

    proptest! {
        /// Preserve every ordered parent directory rustc needs across varied paths.
        #[test]
        fn parser_preserves_loadable_artefact_parent_order(
            artefacts in proptest::collection::vec(artefact_path(), 0..16),
        ) {
            let filenames: Vec<&str> = artefacts.iter().map(|(path, _)| path.as_str()).collect();
            let message = serde_json::json!({
                "reason": "compiler-artifact",
                "target": {"name": "fixture"},
                "filenames": filenames,
            });
            let expected: Vec<PathBuf> = artefacts
                .iter()
                .filter(|(_, loadable)| *loadable)
                .filter_map(|(path, _)| Path::new(path).parent().map(Path::to_path_buf))
                .collect();

            prop_assert_eq!(dependency_dirs_in_message(&message.to_string()), expected);
        }
    }

    #[test]
    fn parser_prefers_metadata_then_falls_back_to_library() {
        let message = r#"{"reason":"compiler-artifact","target":{"name":"fixture"},"filenames":["/final/libfixture.rlib","/build/libfixture.rmeta"]}"#;
        assert_eq!(
            library_path_in_message(message, "fixture"),
            Some(PathBuf::from("/build/libfixture.rmeta")),
            "metadata should be selected when Cargo reports it"
        );
        let rlib_only = r#"{"reason":"compiler-artifact","target":{"name":"fixture"},"filenames":["/final/libfixture.rlib"]}"#;
        assert_eq!(
            library_path_in_message(rlib_only, "fixture"),
            Some(PathBuf::from("/final/libfixture.rlib")),
            "older Cargo layouts need the rlib fallback"
        );
    }
}
