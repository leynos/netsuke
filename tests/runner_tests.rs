//! Unit tests for Ninja process invocation.

use netsuke::cli::{Cli, Commands};
use netsuke::runner;
use rstest::rstest;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

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

struct FakeNinja {
    _dir: TempDir,
    path: PathBuf,
}

impl FakeNinja {
    fn new(exit_code: i32) -> Self {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("ninja");
        let mut file = File::create(&path).expect("script");
        writeln!(file, "#!/bin/sh\nexit {exit_code}").expect("write script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path).expect("meta").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).expect("perms");
        }
        Self { _dir: dir, path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

#[rstest]
#[case(0, true)]
#[case(1, false)]
fn run_ninja_status(#[case] code: i32, #[case] succeeds: bool) {
    let fake = FakeNinja::new(code);
    let cli = test_cli();
    let result = runner::run_ninja(fake.path(), &cli, &[]);
    assert_eq!(result.is_ok(), succeeds);
}

#[rstest]
fn run_ninja_not_found() {
    let cli = test_cli();
    let err =
        runner::run_ninja(Path::new("does-not-exist"), &cli, &[]).expect_err("process should fail");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}
