use netsuke::cli::{Cli, Commands};
use netsuke::runner::{run, run_ninja};
use rstest::rstest;
use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

mod support;

#[test]
fn run_exits_with_manifest_error_on_invalid_version() {
    let temp = tempfile::tempdir().expect("temp dir");
    let manifest_path = temp.path().join("Netsukefile");
    std::fs::copy("tests/data/invalid_version.yml", &manifest_path).expect("copy manifest");
    let cli = Cli {
        file: manifest_path.clone(),
        directory: None,
        jobs: None,
        verbose: false,
        command: Some(Commands::Build { targets: vec![] }),
    };

    let result = run(&cli);
    assert!(result.is_err());
    let err = result.expect_err("should have error");
    assert!(
        err.source()
            .expect("should have source")
            .to_string()
            .contains("version")
    );
}

#[cfg(unix)]
fn fake_ninja_pwd() -> (TempDir, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("ninja");
    let mut file = File::create(&path).expect("script");
    writeln!(
        file,
        "#!/bin/sh\nif [ -n \"$1\" ]; then pwd > \"$1\"; else pwd; fi"
    )
    .expect("write script");
    let mut perms = fs::metadata(&path).expect("meta").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).expect("perms");
    (dir, path)
}

#[cfg(unix)]
#[test]
fn run_executes_ninja_and_captures_logs() {
    let (ninja_dir, ninja_path) = fake_ninja_pwd();
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
        file: manifest_path.clone(),
        directory: Some(temp.path().to_path_buf()),
        jobs: None,
        verbose: false,
        command: Some(Commands::Build { targets: vec![] }),
    };

    let result = run(&cli);
    assert!(result.is_ok());

    // Verify the ninja file was written and contains some content
    let ninja_file_path = temp.path().join("build.ninja");
    assert!(ninja_file_path.exists());
    let ninja_content = std::fs::read_to_string(&ninja_file_path).expect("read ninja file");
    assert!(!ninja_content.is_empty());
    assert!(ninja_content.contains("build "));
    assert!(ninja_content.contains("rule "));

    unsafe {
        std::env::set_var("PATH", original_path);
    } // Nightly marks set_var unsafe.
    drop(ninja_path);
}

#[rstest]
fn run_ninja_not_found() {
    let cli = Cli {
        file: PathBuf::from("/dev/null"),
        directory: None,
        jobs: None,
        verbose: false,
        command: Some(Commands::Build { targets: vec![] }),
    };
    let err = run_ninja(Path::new("does-not-exist"), &cli, &[]).expect_err("process should fail");
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
        file: manifest_path.clone(),
        directory: Some(temp.path().to_path_buf()),
        jobs: None,
        verbose: false,
        command: Some(Commands::Build { targets: vec![] }),
    };

    let result = run(&cli);
    assert!(result.is_ok());

    // Verify the ninja file was written and contains some content
    let ninja_file_path = temp.path().join("build.ninja");
    assert!(ninja_file_path.exists());
    let ninja_content = std::fs::read_to_string(&ninja_file_path).expect("read ninja file");
    assert!(!ninja_content.is_empty());
    assert!(ninja_content.contains("build "));
    assert!(ninja_content.contains("rule "));

    unsafe {
        std::env::set_var("PATH", original_path);
    } // Nightly marks set_var unsafe.
    drop(ninja_path);
}
