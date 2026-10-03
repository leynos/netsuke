//! Guard the runtime-only build-script rerun boundary with source mutations.

use std::io;

use super::{manifest_dir, reject_runtime_only_rerun_paths, static_rerun_paths};

/// Exercise the guard directly so the earlier CLI exact-match check cannot hide a rejection.
#[test]
fn runtime_only_merge_module_rerun_directive_is_rejected() -> io::Result<()> {
    let build_script = test_support::fs::read_to_string(manifest_dir().join("build.rs"))?;
    let normalised_build_script = build_script.replace("\r\n", "\n");
    let original_rerun_paths = static_rerun_paths(&normalised_build_script);
    reject_runtime_only_rerun_paths(&original_rerun_paths)?;

    let mutated_build_script = insert_after_host_pattern_directive(&normalised_build_script)?;
    let mutated_rerun_paths = static_rerun_paths(&mutated_build_script);
    if !mutated_rerun_paths.contains(&"src/cli/merge/mod.rs") {
        return Err(io::Error::other(
            "the mutation did not add src/cli/merge/mod.rs to rerun paths",
        ));
    }

    let rejection = reject_runtime_only_rerun_paths(&mutated_rerun_paths)
        .err()
        .ok_or_else(|| io::Error::other("the runtime-only guard accepted src/cli/merge/mod.rs"))?;
    if !rejection.to_string().contains("src/cli/merge/mod.rs") {
        return Err(io::Error::other(format!(
            "the runtime-only rejection did not identify src/cli/merge/mod.rs: {rejection}",
        )));
    }
    Ok(())
}

/// Insert a runtime-only merge-module directive after the host-pattern directive.
///
/// # Errors
///
/// Returns an error when the host-pattern directive is absent or its indentation
/// cannot be read as a string slice.
fn insert_after_host_pattern_directive(build_script: &str) -> io::Result<String> {
    let mut lines = build_script.split_inclusive('\n');
    let mut mutated_build_script = String::with_capacity(build_script.len());
    for line in lines.by_ref() {
        mutated_build_script.push_str(line);
        if line.trim() == "println!(\"cargo:rerun-if-changed=src/host/pattern.rs\");" {
            let indentation_end = line.len() - line.trim_start().len();
            let indentation = line
                .get(..indentation_end)
                .ok_or_else(|| io::Error::other("could not read the host-pattern indentation"))?;
            if !line.ends_with('\n') {
                mutated_build_script.push('\n');
            }
            mutated_build_script.push_str(indentation);
            mutated_build_script
                .push_str("println!(\"cargo:rerun-if-changed=src/cli/merge/mod.rs\");\n");
            mutated_build_script.push_str(&lines.collect::<String>());
            return Ok(mutated_build_script);
        }
    }
    Err(io::Error::other(
        "build.rs has no src/host/pattern.rs rerun directive",
    ))
}
