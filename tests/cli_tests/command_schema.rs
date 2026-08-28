//! Command-schema coverage for the Clap definitions moved into `cli::command`.
//!
//! These assertions stop at the localized parser boundary. They deliberately do
//! not exercise runner dispatch, so a failing case identifies a schema change
//! rather than runtime command behaviour.

use anyhow::{Context, Result, ensure};
use netsuke::cli::{BuildArgs, Commands, GraphArgs, HelpArgs, HelpTopic};
use netsuke::cli_localization;
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn omitted_subcommand_selects_the_default_build_command() -> Result<()> {
    let localizer = Arc::from(cli_localization::build_localizer(None));
    let (parsed, _) =
        netsuke::cli::parse_with_localizer_from(["netsuke"], &localizer).context("parse CLI")?;
    let command = parsed
        .with_default_command()
        .command
        .context("default command should be present")?;

    ensure!(
        command == Commands::Build(BuildArgs::default()),
        "an omitted subcommand should select the default build command"
    );
    Ok(())
}

#[test]
fn supported_commands_parse_to_their_schema_variants() -> Result<()> {
    let localizer = Arc::from(cli_localization::build_localizer(None));
    let cases = [
        (
            vec!["netsuke", "build", "first", "second"],
            Commands::Build(BuildArgs {
                targets: vec![String::from("first"), String::from("second")],
            }),
        ),
        (vec!["netsuke", "clean"], Commands::Clean),
        (
            vec!["netsuke", "graph", "--html", "--output", "graph.html"],
            Commands::Graph(GraphArgs {
                html: true,
                output: Some(PathBuf::from("graph.html")),
            }),
        ),
        (
            vec!["netsuke", "generate", "--output", "generated.ninja"],
            Commands::Generate {
                output: Some(PathBuf::from("generated.ninja")),
            },
        ),
        (
            vec!["netsuke", "help", "targets"],
            Commands::Help(HelpArgs {
                topic: Some(HelpTopic::Targets),
            }),
        ),
    ];

    for (argv, expected) in cases {
        let (parsed, _) = netsuke::cli::parse_with_localizer_from(argv.clone(), &localizer)
            .with_context(|| format!("parse command schema for {argv:?}"))?;
        let command = parsed
            .with_default_command()
            .command
            .context("parsed command should be present")?;

        ensure!(
            command == expected,
            "command schema mismatch for {argv:?}: got {command:?}, expected {expected:?}"
        );
    }
    Ok(())
}
