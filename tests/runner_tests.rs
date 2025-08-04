//! Unit tests for Ninja process invocation.

use netsuke::cli::{Cli, Commands};
use netsuke::runner;
use rstest::rstest;
use std::path::{Path, PathBuf};

/// Creates a default CLI configuration for testing Ninja invocation.
fn test_cli() -> Cli {
    Cli {
        file: PathBuf::from("Netsukefile"),
        directory: None,
        jobs: None,
        command: Some(Commands::Build {
            targets: Vec::new(),
        }),
    }
}

mod support;

#[rstest]
#[case(0, true)]
#[case(1, false)]
fn run_ninja_status(#[case] code: i32, #[case] succeeds: bool) {
    let (_dir, path) = support::fake_ninja(code);
    let cli = test_cli();
    let result = runner::run_ninja(&path, &cli, &[]);
    assert_eq!(result.is_ok(), succeeds);
}

#[rstest]
fn run_ninja_not_found() {
    let cli = test_cli();
    let err =
        runner::run_ninja(Path::new("does-not-exist"), &cli, &[]).expect_err("process should fail");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}
