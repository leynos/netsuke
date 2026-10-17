//! Exercise build-script rerun inputs with an isolated Cargo fixture.
//!
//! This helper is owned by the split-build case in
//! `locale_stub_ui_tests.rs`. It derives the fixture's environment rerun
//! directives from the production `build.rs`, so that case detects accidental
//! rebuilds caused by inherited Cargo metadata while preserving the
//! `SOURCE_DATE_EPOCH` rebuild contract. Do not reuse it for unrelated Cargo
//! fixtures; their process boundaries and rerun policies may differ.

use std::{io, path::Path};
use test_support::fs as test_fs;

const METADATA_ENVIRONMENT: [(&str, &str); 5] = [
    ("CARGO_PKG_VERSION", "0.1.0"),
    ("CARGO_PKG_DESCRIPTION", "fixture description"),
    ("CARGO_PKG_AUTHORS", "fixture author"),
    ("TARGET", "fixture-target"),
    ("PROFILE", "fixture-profile"),
];

/// Write a fixture build script with the production environment rerun policy.
///
/// For example, a new literal `rerun-if-env-changed` directive in the root
/// build script becomes an input to this nested fixture automatically.
///
/// # Errors
///
/// Returns an error if a production directive cannot be extracted or the
/// fixture build script cannot be written.
pub(crate) fn prepare(root: &Path) -> io::Result<()> {
    let directives = rerun_environment_directives(include_str!("../../build.rs"))?;
    let build_script = format!(
        "fn main() {{\n    println!(\"cargo:rerun-if-changed=build.rs\");\n{directives}}}\n"
    );
    test_fs::write(root.join("fixture_support/build.rs"), build_script)
}

/// Check that metadata changes stay fresh while the source-date input rebuilds.
///
/// The first nested build inherits all five Cargo metadata keys and must reuse
/// the preceding artefact. A second build changes only `SOURCE_DATE_EPOCH` and
/// must rerun, matching the documented reproducible-build input.
///
/// # Errors
///
/// Returns an error when Cargo emits invalid JSON, omits the fixture artefact,
/// or reports freshness that contradicts either expected outcome.
pub(crate) fn verify(root: &Path) -> io::Result<()> {
    let metadata_messages = super::build_split_fixture(root, &METADATA_ENVIRONMENT)?;
    require_freshness(&metadata_messages, true, "the metadata-only fixture build")?;

    let mut source_date_environment = METADATA_ENVIRONMENT.to_vec();
    source_date_environment.push(("SOURCE_DATE_EPOCH", "1"));
    let source_date_messages = super::build_split_fixture(root, &source_date_environment)?;
    require_freshness(
        &source_date_messages,
        false,
        "the SOURCE_DATE_EPOCH fixture build",
    )
}

/// Extract each literal environment rerun directive from the production script.
///
/// For example, a literal directive for `SOURCE_DATE_EPOCH` is returned as a
/// matching `println!` line for the fixture build script.
///
/// # Errors
///
/// Returns an error when a directive is not a supported literal or none exist.
fn rerun_environment_directives(build_script: &str) -> io::Result<String> {
    const PREFIX: &str = "println!(\"cargo:rerun-if-env-changed=";
    const SUFFIX: &str = "\");";

    let mut directives = String::new();
    for (line_index, line) in build_script.lines().enumerate() {
        let Some(directive) = line.trim().strip_prefix(PREFIX) else {
            if line.contains("cargo:rerun-if-env-changed=") {
                return Err(io::Error::other(format!(
                    "build.rs line {} has a non-literal environment rerun directive",
                    line_index + 1,
                )));
            }
            continue;
        };
        let Some(name) = directive.strip_suffix(SUFFIX) else {
            return Err(io::Error::other(format!(
                "build.rs line {} has an unterminated environment rerun directive",
                line_index + 1,
            )));
        };
        if name.is_empty() {
            return Err(io::Error::other(format!(
                "build.rs line {} has an empty environment rerun name",
                line_index + 1,
            )));
        }
        directives.push_str(line);
        directives.push('\n');
    }
    if directives.is_empty() {
        return Err(io::Error::other(
            "build.rs has no literal environment rerun directives",
        ));
    }
    Ok(directives)
}

/// Require the fixture-support artefact to have the expected freshness value.
///
/// For example, requiring `true` rejects a Cargo message showing that the
/// fixture-support artefact was rebuilt.
///
/// # Errors
///
/// Returns an error when the JSON is invalid, the artefact is missing, or its
/// freshness differs from `expected`.
fn require_freshness(messages: &str, expected: bool, operation: &str) -> io::Result<()> {
    let actual = fixture_support_freshness(messages)?;
    if actual != expected {
        return Err(io::Error::other(format!(
            "{operation} should report fixture_support fresh={expected}, found fresh={actual}",
        )));
    }
    Ok(())
}

/// Read the freshness flag from Cargo's fixture-support artefact message.
///
/// For example, a matching compiler-artifact message with `fresh: true` yields
/// `Ok(true)`.
///
/// # Errors
///
/// Returns an error when Cargo emits invalid JSON, omits the matching artefact,
/// or omits its boolean freshness flag.
fn fixture_support_freshness(messages: &str) -> io::Result<bool> {
    let mut freshness = None;
    for (line_index, line) in messages.lines().enumerate() {
        let message: serde_json::Value = serde_json::from_str(line).map_err(|error| {
            io::Error::other(format!(
                "Cargo JSON line {} could not be parsed while checking fixture_support freshness: {error}",
                line_index + 1,
            ))
        })?;
        if message.get("reason").and_then(serde_json::Value::as_str) != Some("compiler-artifact")
            || message
                .get("target")
                .and_then(|target| target.get("name"))
                .and_then(serde_json::Value::as_str)
                != Some("fixture_support")
        {
            continue;
        }
        if super::test_support_rlib::cargo_artifacts::library_path_in_message(
            line,
            "fixture_support",
        )
        .is_none()
        {
            return Err(io::Error::other(format!(
                "Cargo JSON line {} names fixture_support without a library artefact",
                line_index + 1,
            )));
        }
        let is_fresh = message
            .get("fresh")
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| {
                io::Error::other(format!(
                    "Cargo JSON line {} has no boolean freshness for fixture_support",
                    line_index + 1,
                ))
            })?;
        freshness = Some(is_fresh);
    }
    freshness.ok_or_else(|| {
        io::Error::other("Cargo emitted no matching fixture_support compiler-artifact message")
    })
}
