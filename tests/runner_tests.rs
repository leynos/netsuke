//! Unit tests for Ninja process invocation.

use netsuke::cli::{Cli, Commands};
use netsuke::runner;
use rstest::rstest;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::Level;

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
    } // Nightly marks set_var unsafe.

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
    } // Nightly marks set_var unsafe.
    drop(ninja_path);
}

#[rstest]
fn run_ninja_logs_command() {
    let (_dir, path) = support::fake_ninja(0);
    let mut cli = test_cli();
    cli.verbose = true;
    let logs = support::capture_logs(Level::INFO, || {
        runner::run_ninja(&path, &cli, &["--password=123".to_string()]).expect("run");
    });
    assert!(logs.contains("Running command:"));
    assert!(logs.contains("password=***REDACTED***"));
    assert!(!logs.contains("123"));
}

#[rstest]
fn run_with_verbose_mode_emits_logs() {
    let (ninja_dir, ninja_path) = support::fake_ninja(0);
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths: Vec<_> = std::env::split_paths(&original_path).collect();
    paths.insert(0, ninja_dir.path().to_path_buf());
    let new_path = std::env::join_paths(paths).expect("join paths");
    unsafe {
        std::env::set_var("PATH", &new_path);
    } // Nightly marks set_var unsafe.

    let temp = tempfile::tempdir().expect("temp dir");
    let manifest_path = temp.path().join("Netsukefile");
    std::fs::copy("tests/data/minimal.yml", &manifest_path).expect("copy manifest");
    let cli = Cli {
        file: manifest_path,
        directory: Some(temp.path().to_path_buf()),
        jobs: None,
        verbose: true,
        command: Some(Commands::Build {
            targets: Vec::new(),
        }),
    };

    let logs = support::capture_logs(Level::DEBUG, || {
        runner::run(&cli).expect("run");
    });
    assert!(logs.contains("AST:"));
    assert!(logs.contains("Generated Ninja file at"));
    assert!(logs.contains("Running command:"));

    unsafe {
        std::env::set_var("PATH", original_path);
    } // Nightly marks set_var unsafe.
    drop(ninja_path);
}

#[rstest]
fn run_ninja_with_directory() {
    let (root, path) = support::fake_ninja_pwd();
    let workdir = root.path().join("work");
    fs::create_dir(&workdir).expect("workdir");
    let output = root.path().join("out.txt");
    let mut cli = test_cli();
    cli.directory = Some(PathBuf::from("work"));

    let prev = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(root.path()).expect("chdir");
    runner::run_ninja(&path, &cli, &[output.to_string_lossy().to_string()]).expect("ninja run");
    std::env::set_current_dir(prev).expect("restore cwd");

    let recorded = fs::read_to_string(output).expect("read output");
    assert_eq!(recorded.trim(), workdir.to_string_lossy());
}
