//! Contract test: no compiled source suppresses the environment-access policy.
//!
//! `clippy.toml` disallows the process-environment entry points, and the
//! workspace denies `clippy::disallowed_methods`, but the two together do not
//! close the door. An inner attribute — an `allow` of the policy lint, at the
//! top of a file — switches the lint off for everything below it and passes
//! `make lint` with every other contract green: `clippy.toml` still lists the
//! methods, the workspace still denies the lint, and the lint target still runs
//! across the workspace. None of them observes that a source opted out, because
//! each asserts a true statement about something else. A green tick means the
//! gate ran, not that it was allowed to see anything.
//!
//! Clippy cannot close this itself: `clippy::allow_attributes` does not fire on
//! inner attributes, so the seam taxonomy's "an `#[expect]` carrying a reason,
//! never an `allow`" rule has no mechanical enforcement there. The assertion is
//! therefore about source text, which is normally the wrong shape for a
//! contract — but an attribute *is* source text, there is no execution to
//! model, and "this file does not opt out" is exactly a statement about what
//! the file says.
//!
//! [`scanner`] finds the attributes, [`mask`] keeps quoted text out of its way,
//! and [`policy`] decides which names they may carry; this crate holds the walk
//! over the compiled sources and the assertions about it.
//!
//! See `docs/adr-008-environment-seam-taxonomy.md` for the taxonomy, and the
//! developers' guide for the sanctioned forms and the scoped exemption.

use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

#[path = "env_access_suppressions/mask.rs"]
mod mask;
#[path = "env_access_suppressions/policy.rs"]
mod policy;
#[path = "env_access_suppressions/scanner.rs"]
mod scanner;

use scanner::scan_source;

/// Roots, relative to the workspace root, whose Rust sources the crate compiles.
///
/// `tests` is included because `cargo clippy --workspace --all-targets` lints
/// integration-test targets and the modules they wire in, exactly as it lints
/// the library, so an inner attribute there silences the policy for the whole
/// test binary. Measured: with an integration target that reads the
/// environment, the target fails to compile without the attribute and compiles
/// clean with it. Leaving `tests` out would hand the evasion a second home.
const COMPILED_SOURCE_ROOTS: [&str; 4] = ["src", "build_l10n_audit", "test_support/src", "tests"];

/// Compiled sources that sit outside every [`COMPILED_SOURCE_ROOTS`] root.
///
/// `build.rs` is compiled and linted by `cargo clippy --all-targets`, and it is
/// where a build script's own environment reads live, so an inner attribute
/// there silences the policy for the build script exactly as it would in a
/// library source. It is listed separately because the walk is over directories
/// and this is a file.
const STANDALONE_COMPILED_SOURCES: [&str; 1] = ["build.rs"];

/// Append every `.rs` source beneath `directory`, with its contents, in order.
fn collect_rust_sources(
    root: &Dir,
    directory: &str,
    sources: &mut Vec<(String, String)>,
) -> Result<()> {
    for entry_result in root
        .read_dir(directory)
        .with_context(|| format!("read `{directory}`"))?
    {
        let entry = entry_result.with_context(|| format!("read an entry of `{directory}`"))?;
        let name = entry
            .file_name()
            .with_context(|| format!("read an entry name in `{directory}`"))?;
        let path = format!("{directory}/{name}");
        let file_type = entry
            .file_type()
            .with_context(|| format!("read the file type of `{path}`"))?;
        if file_type.is_dir() {
            collect_rust_sources(root, &path, sources)?;
        } else if is_rust_source(&name) {
            let contents = root
                .read_to_string(&path)
                .with_context(|| format!("read {path}"))?;
            sources.push((path, contents));
        }
    }
    Ok(())
}

/// Return whether an entry name is a Rust source.
fn is_rust_source(name: &str) -> bool {
    Utf8Path::new(name).extension().is_some_and(|it| it == "rs")
}

/// Read every compiled source the scan governs, with its workspace-relative path.
fn compiled_sources() -> Result<Vec<(String, String)>> {
    let crate_root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open the workspace root")?;
    let mut sources = Vec::new();
    for root_path in COMPILED_SOURCE_ROOTS {
        collect_rust_sources(&crate_root, root_path, &mut sources)
            .with_context(|| format!("walk the `{root_path}` source root"))?;
    }
    for path in STANDALONE_COMPILED_SOURCES {
        let contents = crate_root
            .read_to_string(path)
            .with_context(|| format!("read {path}"))?;
        sources.push((path.to_owned(), contents));
    }
    Ok(sources)
}

/// Render every finding as one failure message, so one run names them all.
///
/// Collecting before asserting means a contributor sees every offending file in
/// one run rather than fixing them one gate at a time. The message is assembled
/// from a joined list rather than pushed into a growing `String`, because the
/// helper sits outside a `#[test]` body, where neither `expect` nor the
/// `std::fmt::Write` result may be discarded.
fn build_error_message(findings: &[(String, String)]) -> String {
    let listed = findings
        .iter()
        .map(|(path, lint)| format!("{path}: {lint}"))
        .collect::<Vec<_>>()
        .join("\n- ");
    format!(
        "the environment-access policy is suppressed in compiled sources; \
         remove the `allow` and state the site as `#[expect(..., reason = \"...\")]`, \
         or route the access through a seam:\n- {listed}"
    )
}

/// Fail if any compiled source suppresses the environment-access policy.
#[test]
fn compiled_sources_never_suppress_the_environment_policy() -> Result<()> {
    let sources = compiled_sources()?;
    // A sweep that silently matched nothing would make the assertion vacuous.
    ensure!(
        !sources.is_empty(),
        "the compiled source roots should contain at least one Rust source"
    );
    let findings: Vec<(String, String)> = sources
        .iter()
        .flat_map(|(path, contents)| scan_source(path, contents))
        .collect();

    ensure!(findings.is_empty(), "{}", build_error_message(&findings));
    Ok(())
}

#[path = "env_access_suppressions/scanner_tests.rs"]
mod scanner_tests;
