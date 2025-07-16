use clap::Parser;
use clap::error::ErrorKind;
use netsuke::cli::{Cli, Commands};
use rstest::rstest;
use std::path::PathBuf;

#[rstest]
#[case(vec!["netsuke"], PathBuf::from("Netsukefile"), None, None, Commands::Build { targets: Vec::new() })]
#[case(
    vec!["netsuke", "--file", "alt.yml", "-C", "work", "-j", "4", "build", "a", "b"],
    PathBuf::from("alt.yml"),
    Some(PathBuf::from("work")),
    Some(4),
    Commands::Build { targets: vec!["a".into(), "b".into()] },
)]
fn parse_cli(
    #[case] argv: Vec<&str>,
    #[case] file: PathBuf,
    #[case] directory: Option<PathBuf>,
    #[case] jobs: Option<usize>,
    #[case] expected_cmd: Commands,
) {
    let cli = Cli::try_parse_from(argv).expect("parse");
    assert_eq!(cli.file, file);
    assert_eq!(cli.directory, directory);
    assert_eq!(cli.jobs, jobs);
    let command = cli.command.unwrap_or(Commands::Build {
        targets: Vec::new(),
    });
    assert_eq!(command, expected_cmd);
}

#[rstest]
#[case(vec!["netsuke", "unknowncmd"], ErrorKind::InvalidSubcommand)]
#[case(vec!["netsuke", "--file"], ErrorKind::InvalidValue)]
#[case(vec!["netsuke", "-j", "notanumber"], ErrorKind::ValueValidation)]
#[case(vec!["netsuke", "--file", "alt.yml", "-C"], ErrorKind::InvalidValue)]
fn parse_cli_errors(#[case] argv: Vec<&str>, #[case] expected_error: ErrorKind) {
    let err = Cli::try_parse_from(argv).expect_err("unexpected success");
    assert_eq!(err.kind(), expected_error);
}
