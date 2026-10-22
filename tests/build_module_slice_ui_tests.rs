//! Source contracts for the production `build.rs` CLI module slice.
//!
//! Cargo compiles `build_slice_fixture` before test execution. These assertions
//! keep its composition root and rerun inputs aligned with `build.rs`.

#[path = "support/build_module_slice_runtime_only_tests.rs"]
mod runtime_only_rerun_tests;

use std::{io, path::PathBuf};

/// The opening pair that begins the production module slice in either source.
///
/// The fixture marks its `cli` module `pub` and imports `../src/`, so the
/// comparison normalises both spellings to this one before looking for it.
const SLICE_START: &str = "#[path = \"src/cli\"]\nmod cli {";

/// The declarations inside `build.rs`'s inline `cli` module, in order.
const CLI_BLOCK_DECLARATIONS: &[&str] = &[
    "pub mod config;",
    "mod validation;",
    "mod help;",
    "mod command;",
    "pub use command::Cli;",
    "pub use config::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};",
];

/// The path-imported siblings that follow the inline `cli` module, in order.
///
/// Order is part of the contract rather than an accident of formatting: each
/// `#[path = ...]` applies to the declaration beneath it, so a reordered pair
/// would compile a different file while still naming the same modules.
const SIBLING_DECLARATIONS: &[&str] = &[
    "#[path = \"src/cli/localization/mod.rs\"]",
    "mod cli_localization;",
    "#[path = \"src/host/pattern.rs\"]",
    "mod host_pattern;",
    "#[path = \"src/locale/catalogues.rs\"]",
    "pub mod locale_catalogues;",
    "#[path = \"src/localization/mod.rs\"]",
    "pub mod localization;",
];

/// The one declaration `build.rs` carries and the fixture deliberately does
/// not.
///
/// `build_l10n_audit` lives beside `build.rs`, declares no `#[path]`, and walks
/// the generated catalogues at build time rather than exposing production
/// source. It is therefore outside the slice the fixture stands in for, and
/// both halves of that exclusion are asserted rather than left implicit.
const BUILD_SCRIPT_ONLY_DECLARATION: &str = "mod build_l10n_audit;";

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
///
/// Both sources are normalised to one spelling (`../src/` becomes `src/`, and
/// the fixture's `pub mod cli {` becomes `mod cli {`) before their declarations
/// are compared in order. The comparison covers the whole slice rather than the
/// inline `cli` module alone: every path import after it compiles production
/// source that the fixture must mirror. [`BUILD_SCRIPT_ONLY_DECLARATION`] is
/// removed first, because it belongs to the build script and is asserted by its
/// own test.
fn assert_fixture_matches_build_source(build_script: &str) -> io::Result<()> {
    let normalised_build_script = build_script
        .replace("\r\n", "\n")
        .replace("../src/", "src/")
        .replace("pub mod cli {", "mod cli {");
    let slice_start = normalised_build_script
        .find(SLICE_START)
        .ok_or_else(|| io::Error::other("build.rs no longer declares its inline cli module"))?;

    let declared = slice_declarations(&normalised_build_script, slice_start)
        .into_iter()
        .filter(|line| *line != BUILD_SCRIPT_ONLY_DECLARATION)
        .collect::<Vec<_>>();
    let expected = expected_slice_declarations();
    if declared != expected {
        return Err(io::Error::other(format!(
            "build.rs's module slice no longer matches the UI fixture: {declared:?}, \
             expected {expected:?}",
        )));
    }
    Ok(())
}

/// Return the declarations every source of the slice must carry, in order.
fn expected_slice_declarations() -> Vec<&'static str> {
    let mut expected = SLICE_START.lines().collect::<Vec<_>>();
    expected.extend(CLI_BLOCK_DECLARATIONS);
    expected.push("}");
    expected.extend(SIBLING_DECLARATIONS);
    expected
}

/// Collect the slice's declaration lines from `source`, starting at `start`.
///
/// Blank lines and comments between declarations are skipped. The first line
/// that is none of the forms [`is_slice_declaration`] allows ends the slice,
/// because that is where the build script returns to ordinary code — its first
/// attribute that is not a path import, or its first function.
fn slice_declarations(source: &str, start: usize) -> Vec<&str> {
    let mut declarations = Vec::new();
    for line in source.get(start..).unwrap_or_default().lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if !is_slice_declaration(trimmed) {
            break;
        }
        declarations.push(trimmed);
    }
    declarations
}

/// Return whether `line` is one of the declaration forms the slice allows.
///
/// A path attribute belongs to whatever follows it, so it is matched as a line
/// in its own right; the module, re-export, and closing-brace forms are matched
/// by their shape. Nothing here admits a `use` import or an item definition, so
/// an unrelated addition cannot be mistaken for slice membership.
fn is_slice_declaration(line: &str) -> bool {
    let is_module = line.starts_with("mod ") || line.starts_with("pub mod ");
    line == "}"
        || (line.starts_with("#[path = \"") && line.ends_with("\"]"))
        || (is_module && (line.ends_with(';') || line.ends_with('{')))
        || (line.starts_with("pub use ") && line.ends_with(';'))
}

#[test]
fn fixture_contract_accepts_crlf_build_script_source() -> io::Result<()> {
    let mut declarations = expected_slice_declarations();
    let audit_position = declarations
        .iter()
        .position(|line| *line == "#[path = \"src/localization/mod.rs\"]")
        .ok_or_else(|| io::Error::other("the expected slice lost its localization import"))?;
    declarations.insert(audit_position, BUILD_SCRIPT_ONLY_DECLARATION);

    let mut build_script = String::new();
    for line in &declarations {
        build_script.push_str(line);
        build_script.push_str("\r\n");
    }

    assert_fixture_matches_build_source(&build_script)
}

/// Assert the localization audit stays a build-script declaration.
///
/// `build_l10n_audit` has no `#[path]`, lives beside `build.rs`, and audits the
/// generated catalogues rather than exposing production source. The fixture
/// package has no such module, so declaring it there would not compile — but
/// copying it would also widen what the fixture claims to stand in for. Both
/// halves are asserted so the exclusion reads as a decision rather than an
/// omission.
#[test]
fn the_localization_audit_stays_out_of_the_production_slice_fixture() -> io::Result<()> {
    let build_script = test_support::fs::read_to_string(manifest_dir().join("build.rs"))?;
    if !build_script.contains(BUILD_SCRIPT_ONLY_DECLARATION) {
        return Err(io::Error::other(format!(
            "build.rs must keep declaring {BUILD_SCRIPT_ONLY_DECLARATION}",
        )));
    }

    let fixture =
        test_support::fs::read_to_string(manifest_dir().join("build_slice_fixture/slice.rs"))?;
    if fixture.contains(BUILD_SCRIPT_ONLY_DECLARATION) {
        return Err(io::Error::other(format!(
            "slice.rs must not declare {BUILD_SCRIPT_ONLY_DECLARATION}; it is build-script-only \
             and has no counterpart in the fixture package",
        )));
    }
    Ok(())
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
