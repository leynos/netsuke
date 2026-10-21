//! Source contracts for the production `build.rs` CLI module slice.
//!
//! Cargo compiles `build_slice_fixture` before test execution. These assertions
//! keep its composition root and rerun inputs aligned with `build.rs`.

#[path = "support/build_module_slice_runtime_only_tests.rs"]
mod runtime_only_rerun_tests;

use std::{io, path::PathBuf};

/// The exact module declarations that the fixture mirrors from `build.rs`.
const BUILD_SLICE_MODULES: &[&str] = &[
    "pub mod config;",
    "mod validation;",
    "mod help;",
    "mod command;",
];

/// The CLI source paths that the build-script facade compiles and tracks.
const BUILD_SLICE_RERUN_PATHS: &[&str] = &[
    "src/cli/command.rs",
    "src/cli/config/mod.rs",
    "src/cli/config/validation.rs",
    "src/cli/config/budget.rs",
    "src/cli/config/no_input.rs",
    "src/cli/config/policy_definitions.rs",
    "src/cli/help.rs",
    "src/cli/validation.rs",
];

/// Runtime-only modules that must not widen the build-script module slice.
const RUNTIME_ONLY_RERUN_PATHS: &[&str] = &[
    "src/cli/diag/mod.rs",
    "src/cli/discovery/mod.rs",
    "src/cli/merge/mod.rs",
    "src/cli/parser/mod.rs",
    "src/cli/parsing.rs",
    "src/host/matching.rs",
];

#[test]
fn runtime_only_rerun_paths_exist() -> io::Result<()> {
    let repository_root = manifest_dir();
    let missing_paths = RUNTIME_ONLY_RERUN_PATHS
        .iter()
        .filter(|path| !repository_root.join(**path).is_file())
        .copied()
        .collect::<Vec<_>>();
    if !missing_paths.is_empty() {
        return Err(io::Error::other(format!(
            "the runtime-only rerun path list is stale; missing files: {missing_paths:?}",
        )));
    }
    Ok(())
}

#[test]
fn production_build_module_slice_has_expected_boundary() -> io::Result<()> {
    assert_fixture_matches_build_rs()?;
    let fixture =
        test_support::fs::read_to_string(manifest_dir().join("build_slice_fixture/slice.rs"))?;
    assert_fixture_matches_build_source(&fixture)
}

/// Verify the fixture root still mirrors the module declarations in `build.rs`.
fn assert_fixture_matches_build_rs() -> io::Result<()> {
    let build_script = test_support::fs::read_to_string(manifest_dir().join("build.rs"))?;
    assert_fixture_matches_build_source(&build_script)
}

/// Verify one build-script source text declares exactly the fixture module slice.
fn assert_fixture_matches_build_source(build_script: &str) -> io::Result<()> {
    let normalised_build_script = build_script
        .replace("\r\n", "\n")
        .replace("../src/", "src/")
        .replace("pub mod cli {", "mod cli {");
    let slice_start = normalised_build_script
        .find("#[path = \"src/cli\"]\nmod cli {")
        .ok_or_else(|| io::Error::other("build.rs no longer declares its inline cli module"))?;
    let declared_slice = normalised_build_script
        .get(slice_start..)
        .and_then(|slice| {
            slice
                .find("#[path = \"src/cli/localization/mod.rs\"]")
                .and_then(|slice_end| slice.get(..slice_end))
        })
        .ok_or_else(|| {
            io::Error::other("could not locate the end of build.rs's cli module slice")
        })?;

    let mut declared_modules = declared_slice
        .lines()
        .map(str::trim)
        .filter(|line| {
            (line.starts_with("mod ") || line.starts_with("pub mod ")) && line.ends_with(';')
        })
        .collect::<Vec<_>>();
    declared_modules.sort_unstable();
    let mut expected_modules = BUILD_SLICE_MODULES.to_vec();
    expected_modules.sort_unstable();
    if declared_modules != expected_modules {
        return Err(io::Error::other(format!(
            "build.rs's cli slice no longer matches the UI fixture: {declared_modules:?}",
        )));
    }
    if declared_slice.matches("#[path = ").count() != 1 {
        return Err(io::Error::other(
            "build.rs's cli slice contains a different set of path modules than the UI fixture",
        ));
    }
    Ok(())
}

#[test]
fn fixture_contract_accepts_crlf_build_script_source() -> io::Result<()> {
    let mut build_script = String::from("#[path = \"src/cli\"]\r\nmod cli {\r\n");
    for declaration in BUILD_SLICE_MODULES {
        build_script.push_str("    ");
        build_script.push_str(declaration);
        build_script.push_str("\r\n");
    }
    build_script.push_str("}\r\n#[path = \"src/cli/localization/mod.rs\"]\r\n");

    assert_fixture_matches_build_source(&build_script)
}

/// Reject a runtime child added to the build-script module slice.
#[test]
fn fixture_contract_rejects_extra_plain_child_module() -> io::Result<()> {
    let build_script = test_support::fs::read_to_string(manifest_dir().join("build.rs"))?;
    let widened_slice = build_script.replacen(
        "    mod command;",
        "    mod command;\n    mod discovery;",
        1,
    );

    let error = assert_fixture_matches_build_source(&widened_slice)
        .expect_err("an extra runtime module must widen the declared slice");
    if !error
        .to_string()
        .contains("no longer matches the UI fixture")
    {
        return Err(io::Error::other(format!(
            "unexpected fixture mismatch: {error}",
        )));
    }
    Ok(())
}

/// Verify rerun directives track only the build-script's compiled module slice.
#[test]
fn build_script_rerun_directives_match_the_compiled_module_slice() -> io::Result<()> {
    let build_script = test_support::fs::read_to_string(manifest_dir().join("build.rs"))?;
    let normalised_build_script = build_script.replace("\r\n", "\n");
    let rerun_paths = static_rerun_paths(&normalised_build_script);
    let cli_rerun_paths = rerun_paths
        .iter()
        .filter(|path| path.starts_with("src/cli/"))
        .copied()
        .collect::<Vec<_>>();

    if cli_rerun_paths.as_slice() != BUILD_SLICE_RERUN_PATHS {
        return Err(io::Error::other(format!(
            "build.rs's CLI rerun directives no longer match the compiled slice: {cli_rerun_paths:?}",
        )));
    }
    if rerun_paths
        .iter()
        .filter(|path| **path == "src/host/pattern.rs")
        .count()
        != 1
    {
        return Err(io::Error::other(
            "build.rs must track src/host/pattern.rs exactly once",
        ));
    }
    reject_runtime_only_rerun_paths(&rerun_paths)?;
    Ok(())
}

/// Reject build-script rerun directives that track runtime-only modules.
fn reject_runtime_only_rerun_paths(rerun_paths: &[&str]) -> io::Result<()> {
    for &path in RUNTIME_ONLY_RERUN_PATHS {
        if rerun_paths.contains(&path) {
            return Err(io::Error::other(format!(
                "build.rs must not track runtime-only module {path:?}",
            )));
        }
    }
    Ok(())
}

/// Return static source paths emitted by `build.rs` as rerun directives.
fn static_rerun_paths(build_script: &str) -> Vec<&str> {
    build_script
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("println!(\"cargo:rerun-if-changed=")
                .and_then(|directive| directive.strip_suffix("\");"))
        })
        .collect()
}

/// Return the repository root supplied by Cargo for this test target.
fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}
