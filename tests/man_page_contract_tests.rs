//! Contract tests for the manual page emitted by `build.rs`.
//!
//! The crates.io package is `netsuke-build`, while the command, the library,
//! and the binary are all `netsuke`. The build script must name and stamp the
//! manual page from the command-line interface (CLI) name alone, because
//! packaging, the Debian and RPM payloads, and `man netsuke` all expect
//! `netsuke.1`. These tests pin that decoupling so a manual page named after
//! the Cargo package cannot pass unnoticed.

use anyhow::{Context, Result, ensure};
use clap::CommandFactory;
use netsuke::cli::Cli;
use std::path::{Component, Path, PathBuf};
use test_support::fs as test_fs;

/// Cargo package name, deliberately distinct from the target names.
const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
/// Package version stamped into the manual page's source field.
const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Manual page path published by `build.rs` via `cargo:rustc-env`.
const GENERATED_MAN_PAGE: &str = env!("NETSUKE_GENERATED_MAN_PAGE");

/// Resolve the generated manual page against the package root.
///
/// `build.rs` emits a path relative to the package root; joining an absolute
/// path is a no-op, so this also tolerates an absolute emission.
fn generated_man_page() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(GENERATED_MAN_PAGE)
}

/// Return the command name clap renders the manual page for.
fn cli_name() -> String {
    let command = Cli::command();
    command
        .get_bin_name()
        .unwrap_or_else(|| command.get_name())
        .to_owned()
}

/// Return the `.TH` header line of a rendered manual page.
fn title_header(page: &str) -> Result<String> {
    page.lines()
        .find(|line| line.starts_with(".TH "))
        .map(str::trim_end)
        .map(ToOwned::to_owned)
        .context("rendered manual page should carry a `.TH` header")
}

#[test]
fn package_and_target_names_diverge() {
    // Without this divergence the remaining assertions would hold trivially.
    assert_eq!(PACKAGE_NAME, "netsuke-build", "Cargo package name drifted");
    assert_eq!(cli_name(), "netsuke", "CLI command name drifted");
}

#[test]
fn manual_page_is_named_for_the_command_not_the_package() -> Result<()> {
    let path = generated_man_page();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("generated manual page should have a UTF-8 file name")?;
    ensure!(
        file_name == format!("{name}.1", name = cli_name()),
        "manual page {file_name} should be named for the command"
    );
    ensure!(
        file_name == "netsuke.1",
        "packaging and `man netsuke` expect netsuke.1, found {file_name}"
    );
    ensure!(
        file_name != format!("{PACKAGE_NAME}.1"),
        "manual page must not be named after the Cargo package"
    );
    Ok(())
}

#[test]
fn manual_page_is_staged_under_the_target_and_profile_directory() -> Result<()> {
    let relative = Path::new(GENERATED_MAN_PAGE);
    let components: Vec<_> = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect();
    // target/generated-man/<target>/<profile>/netsuke.1
    ensure!(
        components.len() >= 5,
        "manual page path {GENERATED_MAN_PAGE} should carry target and profile directories"
    );
    let prefix = components
        .get(..2)
        .context("manual page path should start with a staging directory")?;
    ensure!(
        prefix == ["target", "generated-man"],
        "manual page should stage under target/generated-man, found {GENERATED_MAN_PAGE}"
    );
    Ok(())
}

#[test]
fn manual_page_source_is_stamped_with_the_command_name() -> Result<()> {
    let path = generated_man_page();
    let page = test_fs::read_to_string(&path)
        .with_context(|| format!("read generated manual page {}", path.display()))?;
    let header = title_header(&page)?;
    let name = cli_name();
    ensure!(
        header.starts_with(&format!(".TH {name} 1 ")),
        "manual page title should announce the command and section 1: {header}"
    );
    ensure!(
        header.contains(&format!("\"{name} {PACKAGE_VERSION}\"")),
        "manual page source should read `{name} {PACKAGE_VERSION}`: {header}"
    );
    ensure!(
        !header.contains(PACKAGE_NAME),
        "manual page title must not surface the Cargo package name: {header}"
    );
    Ok(())
}

#[test]
fn manual_page_documents_the_help_targets_topic() -> Result<()> {
    let path = generated_man_page();
    let page = test_fs::read_to_string(&path)
        .with_context(|| format!("read generated manual page {}", path.display()))?;

    ensure!(
        page.contains("netsuke\\-help(1)"),
        "manual page should list the help command: {page}"
    );
    ensure!(
        page.contains("help targets"),
        "manual page should document the targets topic: {page}"
    );
    Ok(())
}
