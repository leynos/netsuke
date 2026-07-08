//! Typed verification helpers for CLI-focused BDD step definitions.
//!
//! Split from `cli.rs` so both files stay within the module size budget; the
//! step definitions in the parent module import and call these helpers.

use super::{
    cli_network_policy, extract_build, extract_graph_args, extract_manifest_command_file,
    get_command,
};
use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::helpers::assertions::normalize_fluent_isolates;
use crate::bdd::types::{ErrorFragment, JobCount, PathString, TargetName, UrlString};
use anyhow::{Context, Result, bail, ensure};
use netsuke::cli::Commands;
use std::path::PathBuf;

/// Expected CLI command variants for verification.
#[derive(Copy, Clone)]
pub(super) enum ExpectedCommand {
    Build,
    Clean,
    Graph,
    Manifest,
}

impl ExpectedCommand {
    /// Check if the actual command matches the expected variant.
    #[expect(
        clippy::missing_const_for_fn,
        reason = "Commands contains heap-allocated types preventing const evaluation"
    )]
    fn matches(self, actual: &Commands) -> bool {
        matches!(
            (self, actual),
            (Self::Build, Commands::Build(_))
                | (Self::Clean, Commands::Clean)
                | (Self::Graph, Commands::Graph(_))
                | (Self::Manifest, Commands::Manifest { .. })
        )
    }

    /// Return the command name for error messages.
    const fn name(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Clean => "clean",
            Self::Graph => "graph",
            Self::Manifest => "manifest",
        }
    }
}

pub(super) fn verify_command(world: &TestWorld, expected: ExpectedCommand) -> Result<()> {
    let command = get_command(world)?;
    ensure!(
        expected.matches(&command),
        "command should be {}",
        expected.name()
    );
    Ok(())
}

pub(super) fn verify_job_count(world: &TestWorld, expected: JobCount) -> Result<()> {
    let actual = world
        .cli
        .with_ref(|cli| cli.jobs)
        .context("CLI has not been parsed")?;
    ensure!(
        actual == Some(expected.value()),
        "expected job count {}, got {:?}",
        expected.value(),
        actual
    );
    Ok(())
}

pub(super) fn verify_parsing_succeeded(world: &TestWorld) -> Result<()> {
    ensure!(world.cli.is_some(), "CLI should be present after parsing");
    Ok(())
}

pub(super) fn verify_error_returned(world: &TestWorld) -> Result<()> {
    ensure!(
        world.cli_error.is_filled(),
        "Expected an error, but none was returned"
    );
    Ok(())
}

pub(super) fn verify_manifest_path(world: &TestWorld, path: &PathString) -> Result<()> {
    let actual = world
        .cli
        .with_ref(|cli| cli.file.clone())
        .context("CLI has not been parsed")?;
    ensure!(
        actual.as_path() == path.as_path(),
        "expected manifest path {}, got {}",
        path,
        actual.display()
    );
    Ok(())
}

pub(super) fn verify_first_target(world: &TestWorld, target: &TargetName) -> Result<()> {
    let (targets, _) = extract_build(world)?;
    ensure!(
        targets.first().map(String::as_str) == Some(target.as_str()),
        "expected first target {}, got {:?}",
        target,
        targets.first()
    );
    Ok(())
}

/// Assert that an optional path equals the expected value.
fn ensure_optional_path(actual: Option<PathBuf>, expected: &PathString, label: &str) -> Result<()> {
    ensure!(
        actual.as_deref() == Some(expected.as_path()),
        "expected {label} {expected}, got {actual:?}",
    );
    drop(actual);
    Ok(())
}

pub(super) fn verify_working_directory(world: &TestWorld, directory: &PathString) -> Result<()> {
    let actual = world
        .cli
        .with_ref(|cli| cli.directory.clone())
        .context("CLI has not been parsed")?;
    ensure_optional_path(actual, directory, "working directory")
}

pub(super) fn verify_emit_path(world: &TestWorld, path: &PathString) -> Result<()> {
    let (_, emit) = extract_build(world)?;
    ensure_optional_path(emit, path, "emit path")
}

pub(super) fn verify_cli_policy_allows(world: &TestWorld, url: &UrlString) -> Result<()> {
    let policy = cli_network_policy(world)?;
    let parsed = url.parse().context("parse URL for CLI policy check")?;
    ensure!(
        policy.evaluate(&parsed).is_ok(),
        "expected CLI policy to allow {}",
        url,
    );
    Ok(())
}

pub(super) fn verify_cli_policy_rejects(
    world: &TestWorld,
    url: &UrlString,
    message: &ErrorFragment,
) -> Result<()> {
    let policy = cli_network_policy(world)?;
    let parsed = url.parse().context("parse URL for CLI policy check")?;
    let Err(err) = policy.evaluate(&parsed) else {
        bail!("expected CLI policy to reject {}", url);
    };
    let normalized_error = normalize_fluent_isolates(&err.to_string());
    let normalized_message = normalize_fluent_isolates(message.as_str());
    ensure!(
        normalized_error.contains(&normalized_message),
        "expected error to mention '{}', got '{err}'",
        message,
    );
    Ok(())
}

pub(super) fn verify_graph_output_path(world: &TestWorld, path: &PathString) -> Result<()> {
    let output = extract_graph_args(world)?.output;
    ensure_optional_path(output, path, "graph output path")
}

pub(super) fn verify_graph_html_set(world: &TestWorld) -> Result<()> {
    let args = extract_graph_args(world)?;
    ensure!(args.html, "expected graph --html to be set");
    Ok(())
}

pub(super) fn verify_manifest_command_path(world: &TestWorld, path: &PathString) -> Result<()> {
    let file = extract_manifest_command_file(world)?;
    ensure!(
        file == path.to_path_buf(),
        "expected manifest output {}, got {}",
        path,
        file.display()
    );
    Ok(())
}

pub(super) fn verify_error_contains(world: &TestWorld, fragment: &ErrorFragment) -> Result<()> {
    let error = world
        .cli_error
        .get()
        .context("no error was returned by CLI parsing")?;
    let normalized_error = normalize_fluent_isolates(&error);
    let normalized_fragment = normalize_fluent_isolates(fragment.as_str());
    ensure!(
        normalized_error.contains(&normalized_fragment),
        "Error message '{error}' does not contain expected '{}'",
        fragment
    );
    Ok(())
}
