//! Contract tests for shell completions generated from Netsuke's Clap command tree.

use anyhow::{Context, Result, ensure};
use clap::CommandFactory;
use netsuke::cli::Cli;
use rstest::rstest;
use std::path::{Path, PathBuf};
use test_support::fs as test_fs;

/// Directory published by `build.rs` after generating the completion files.
const GENERATED_COMPLETIONS_DIR: &str = env!("NETSUKE_GENERATED_COMPLETIONS_DIR");

/// Resolve the generated completion directory against the package root.
fn generated_completions_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(GENERATED_COMPLETIONS_DIR)
}

/// Collect the command and option terms every generated completion must expose.
fn cli_completion_terms() -> Vec<String> {
    let command = Cli::command();
    let mut terms = command
        .get_subcommands()
        .map(|subcommand| subcommand.get_name().to_owned())
        .collect::<Vec<_>>();
    if let Some(help_command) = command
        .get_subcommands()
        .find(|subcommand| subcommand.get_name() == "help")
    {
        terms.extend(
            help_command
                .get_subcommands()
                .map(|topic| topic.get_name().to_owned()),
        );
    }
    terms.extend(
        command
            .get_arguments()
            .filter_map(|argument| argument.get_long().map(ToOwned::to_owned)),
    );
    terms
}

#[rstest]
#[case("netsuke.bash")]
#[case("netsuke.elv")]
#[case("netsuke.fish")]
#[case("_netsuke.ps1")]
#[case("_netsuke")]
fn generated_completion_exposes_the_clap_command_tree(#[case] file_name: &str) -> Result<()> {
    let path = generated_completions_dir().join(file_name);
    let completion = test_fs::read_to_string(&path)
        .with_context(|| format!("read generated completion {}", path.display()))?;

    for topic in ["help", "targets"] {
        ensure!(
            completion.contains(topic),
            "generated completion {file_name} should expose the {topic:?} help topic: {completion}"
        );
    }
    for term in cli_completion_terms() {
        ensure!(
            completion.contains(&term),
            "generated completion {file_name} should expose {term:?}: {completion}"
        );
    }
    Ok(())
}

/// Verifies generators that support possible values retain the policy spellings.
#[rstest]
#[case("netsuke.bash")]
#[case("netsuke.fish")]
#[case("_netsuke")]
fn generated_completion_exposes_policy_values(#[case] file_name: &str) -> Result<()> {
    let path = generated_completions_dir().join(file_name);
    let completion = test_fs::read_to_string(&path)
        .with_context(|| format!("read generated completion {}", path.display()))?;

    for value in ["auto", "always", "never", "on", "off"] {
        ensure!(
            completion.contains(value),
            "generated completion {file_name} should expose policy value {value:?}: {completion}"
        );
    }
    Ok(())
}
