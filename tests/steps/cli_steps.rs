use crate::CliWorld;
use clap::Parser;
use cucumber::{then, when};
use netsuke::cli::{Cli, Commands};
use std::path::PathBuf;

#[allow(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[when(expr = "the CLI is parsed with {string}")]
fn parse_cli(world: &mut CliWorld, args: String) {
    let tokens: Vec<String> = if args.is_empty() {
        vec!["netsuke".to_string()]
    } else {
        std::iter::once("netsuke".to_string())
            .chain(args.split_whitespace().map(str::to_string))
            .collect()
    };
    world.cli = Some(Cli::parse_from(tokens));
}

#[then("parsing succeeds")]
fn parsing_succeeds(world: &mut CliWorld) {
    assert!(world.cli.is_some());
}

#[then("the command is build")]
fn command_is_build(world: &mut CliWorld) {
    let cli = world.cli.as_ref().expect("cli");
    match cli
        .command
        .clone()
        .unwrap_or(Commands::Build { targets: vec![] })
    {
        Commands::Build { .. } => (),
        _ => panic!("not build"),
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the manifest path is {string}")]
fn manifest_path(world: &mut CliWorld, path: String) {
    let cli = world.cli.as_ref().expect("cli");
    assert_eq!(cli.file, PathBuf::from(path));
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Cucumber requires owned String arguments"
)]
#[then(expr = "the first target is {string}")]
fn first_target(world: &mut CliWorld, target: String) {
    let cli = world.cli.as_ref().expect("cli");
    let cmd = cli
        .command
        .clone()
        .unwrap_or(Commands::Build { targets: vec![] });
    match cmd {
        Commands::Build { targets } => assert_eq!(targets.first(), Some(&target)),
        _ => panic!("expected build"),
    }
}
