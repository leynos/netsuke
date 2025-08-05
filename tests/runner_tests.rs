//! Unit tests for Ninja process invocation.
//!
//! These tests verify that the runner can translate a manifest into a Ninja
//! build script and invoke the Ninja process with appropriate arguments.

use netsuke::cli::{Cli, Commands};
use netsuke::runner;
use rstest::rstest;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

/// Creates a default CLI configuration for testing Ninja invocation.
fn cli_with_manifest(file: PathBuf) -> Cli {
    Cli {
        file,
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
    let mut manifest = NamedTempFile::new().expect("manifest");
    support::write_manifest(&mut manifest);
    let cli = cli_with_manifest(manifest.path().to_path_buf());
    let result = runner::run_ninja(&path, &cli, &[]);
    assert_eq!(result.is_ok(), succeeds);
}

#[rstest]
fn run_ninja_not_found() {
    let mut manifest = NamedTempFile::new().expect("manifest");
    support::write_manifest(&mut manifest);
    let cli = cli_with_manifest(manifest.path().to_path_buf());
    let err =
        runner::run_ninja(Path::new("does-not-exist"), &cli, &[]).expect_err("process should fail");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[rstest]
fn run_pipeline_generates_ninja() {
    use netsuke::ast::Recipe;
    use netsuke::hasher::ActionHasher;
    use netsuke::ir::Action;

    let (_dir, path, capture) = support::fake_ninja_capture();
    let mut manifest = NamedTempFile::new().expect("manifest");
    support::write_manifest(&mut manifest);
    let cli = cli_with_manifest(manifest.path().to_path_buf());

    runner::run_ninja(&path, &cli, &[]).expect("run ninja");

    let generated = fs::read_to_string(&capture).expect("captured build");

    let action = Action {
        recipe: Recipe::Command {
            command: "echo hi".into(),
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let hash = ActionHasher::hash(&action);
    let expected =
        format!("rule {hash}\n  command = echo hi\n\nbuild out: {hash}\n\ndefault out\n");
    assert_eq!(generated, expected);
}
