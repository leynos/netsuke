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
        verbose: false,
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

#[rstest]
fn run_writes_ninja_file() {
    let (ninja_dir, ninja_path) = support::fake_ninja(0);
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths: Vec<_> = std::env::split_paths(&original_path).collect();
    paths.insert(0, ninja_dir.path().to_path_buf());
    let new_path = std::env::join_paths(paths).expect("join paths");
    unsafe {
        std::env::set_var("PATH", &new_path);
    }

    let temp = tempfile::tempdir().expect("temp dir");
    let manifest_path = temp.path().join("Netsukefile");
    std::fs::copy("tests/data/minimal.yml", &manifest_path).expect("copy manifest");
    let cli = Cli {
        file: manifest_path,
        directory: Some(temp.path().to_path_buf()),
        jobs: None,
        verbose: false,
        command: Some(Commands::Build {
            targets: Vec::new(),
        }),
    };

    runner::run(&cli).expect("run");
    assert!(temp.path().join("build.ninja").exists());

    unsafe {
        std::env::set_var("PATH", original_path);
    }
    drop(ninja_path);
}
