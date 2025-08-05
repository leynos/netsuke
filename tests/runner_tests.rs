//! Unit tests for Ninja process invocation.

use netsuke::cli::{Cli, Commands};
use netsuke::runner;
use rstest::rstest;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tracing::Level;

#[cfg(unix)]
mod support;

#[cfg(unix)]
use serial_test::serial;

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

#[cfg(unix)]
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

#[cfg(unix)]
#[rstest]
#[serial]
#[case(false)]
#[case(true)]
fn run_ninja_with_directory(#[case] absolute: bool) {
    let (root, path) = fake_ninja_pwd();
    let workdir = root.path().join("work");
    fs::create_dir(&workdir).expect("workdir");
    let output = root.path().join("out.txt");
    let mut cli = test_cli();
    cli.directory = Some(if absolute {
        workdir.clone()
    } else {
        PathBuf::from("work")
    });

    let prev = std::env::current_dir().expect("cwd");
    if !absolute {
        std::env::set_current_dir(root.path()).expect("chdir");
    }
    runner::run_ninja(&path, &cli, &[output.to_string_lossy().to_string()]).expect("ninja run");

    let recorded = fs::read_to_string(&output).expect("read output");
    let expected = fs::canonicalize(&workdir).expect("canon workdir");

    if !absolute {
        std::env::set_current_dir(prev).expect("restore cwd");
    }
    drop(root); // ensure tempdir outlives any `chdir`

    assert_eq!(recorded.trim(), expected.to_string_lossy());
}

#[cfg(unix)]
fn fake_ninja_pwd() -> (TempDir, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("ninja");
    let mut file = File::create(&path).expect("script");
    writeln!(file, "#!/bin/sh\npwd > \"$1\"").expect("write script");
    let mut perms = fs::metadata(&path).expect("meta").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).expect("perms");
    (dir, path)
}
