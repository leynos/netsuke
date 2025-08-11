use mockable::{DefaultEnv, MockEnv};
use netsuke::cli::{BuildArgs, Cli, Commands};
use netsuke::runner::{BuildTargets, NINJA_ENV, run, run_ninja};
use rstest::{fixture, rstest};
use serial_test::serial;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

mod support;

/// Guard that restores PATH to its original value when dropped.
///
/// Using a simple guard avoids heap allocation and guarantees teardown on
/// early returns or panics.
struct PathGuard {
    original: OsString,
}

impl PathGuard {
    fn new(original: OsString) -> Self {
        Self { original }
    }
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        // Nightly marks set_var unsafe.
        unsafe { std::env::set_var("PATH", &self.original) };
    }
}

/// Fixture: Put a fake `ninja` (that checks for a build file) on PATH.
///
/// Returns: (tempdir holding ninja, `ninja_path`, PATH guard)
#[fixture]
fn ninja_in_path() -> (tempfile::TempDir, PathBuf, PathGuard) {
    let (ninja_dir, ninja_path) = support::fake_ninja_check_build_file();

    // Save PATH and prepend our fake ninja directory.
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths: Vec<_> = std::env::split_paths(&original_path).collect();
    paths.insert(0, ninja_dir.path().to_path_buf());
    let new_path = std::env::join_paths(paths).expect("join paths");
    // Nightly marks set_var unsafe.
    unsafe { std::env::set_var("PATH", &new_path) };

    let guard = PathGuard::new(original_path);
    (ninja_dir, ninja_path, guard)
}

/// Fixture: Put a fake `ninja` with a specific exit code on PATH.
///
/// The default exit code is 0, but can be customised via `#[with(...)]`.
///
/// Returns: (tempdir holding ninja, `ninja_path`, PATH guard)
#[fixture]
fn ninja_with_exit_code(#[default(0)] exit_code: i32) -> (tempfile::TempDir, PathBuf, PathGuard) {
    let (ninja_dir, ninja_path) = support::fake_ninja(exit_code);

    // Save PATH and prepend our fake ninja directory.
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths: Vec<_> = std::env::split_paths(&original_path).collect();
    paths.insert(0, ninja_dir.path().to_path_buf());
    let new_path = std::env::join_paths(paths).expect("join paths");
    // Nightly marks set_var unsafe.
    unsafe { std::env::set_var("PATH", &new_path) };

    let guard = PathGuard::new(original_path);
    (ninja_dir, ninja_path, guard)
}

/// Fixture: Create a temporary project with a Netsukefile from minimal.yml.
///
/// Returns: (tempdir for project, path to Netsukefile)
#[fixture]
fn test_manifest() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("temp dir");
    let manifest_path = temp.path().join("Netsukefile");
    std::fs::copy("tests/data/minimal.yml", &manifest_path).expect("copy manifest");
    (temp, manifest_path)
}

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
        command: Some(Commands::Build(BuildArgs {
            emit: None,
            targets: vec![],
        })),
    };
    let env = DefaultEnv::new();
    let result = run(&cli, &env);
    assert!(result.is_err());
    let err = result.expect_err("should have error");
    assert!(err.chain().any(|e| e.to_string().contains("version")));
}

#[rstest]
fn run_ninja_not_found() {
    let cli = Cli {
        file: PathBuf::from("/dev/null"),
        directory: None,
        jobs: None,
        verbose: false,
        command: Some(Commands::Build(BuildArgs {
            emit: None,
            targets: vec![],
        })),
    };
    let targets = BuildTargets::default();
    let err = run_ninja(
        Path::new("does-not-exist"),
        &cli,
        Path::new("build.ninja"),
        &targets,
    )
    .expect_err("process should fail");
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[rstest]
#[serial]
fn run_executes_ninja_without_persisting_file() {
    let (_ninja_dir, ninja_path, _guard) = ninja_in_path();
    let (temp, manifest_path) = test_manifest();
    let cli = Cli {
        file: manifest_path.clone(),
        directory: Some(temp.path().to_path_buf()),
        jobs: None,
        verbose: false,
        command: Some(Commands::Build(BuildArgs {
            emit: None,
            targets: vec![],
        })),
    };
    let env = DefaultEnv::new();
    let result = run(&cli, &env);
    assert!(result.is_ok());

    // Ensure no ninja file remains in project directory
    assert!(!temp.path().join("build.ninja").exists());

    // Drop the fake ninja artifacts. PATH is restored by guard drop.
    drop(ninja_path);
}

#[cfg(unix)]
#[serial]
#[rstest]
fn run_build_with_emit_keeps_file() {
    let (_ninja_dir, ninja_path, _guard) = ninja_in_path();
    let (temp, manifest_path) = test_manifest();
    let emit_path = temp.path().join("emitted.ninja");
    let cli = Cli {
        file: manifest_path.clone(),
        directory: Some(temp.path().to_path_buf()),
        jobs: None,
        verbose: false,
        command: Some(Commands::Build(BuildArgs {
            emit: Some(emit_path.clone()),
            targets: vec![],
        })),
    };
    let env = DefaultEnv::new();
    let result = run(&cli, &env);
    assert!(result.is_ok());

    assert!(emit_path.exists());
    let emitted = std::fs::read_to_string(&emit_path).expect("read emitted");
    assert!(emitted.contains("rule "));
    assert!(emitted.contains("build "));
    assert!(!temp.path().join("build.ninja").exists());

    // Drop the fake ninja artifacts. PATH is restored by guard drop.
    drop(ninja_path);
}

#[cfg(unix)]
#[serial]
#[rstest]
fn run_build_with_emit_creates_parent_dirs() {
    let (_ninja_dir, ninja_path, _guard) = ninja_with_exit_code(0);
    let (temp, manifest_path) = test_manifest();
    let nested_dir = temp.path().join("nested").join("dir");
    let emit_path = nested_dir.join("emitted.ninja");
    assert!(!nested_dir.exists());
    let cli = Cli {
        file: manifest_path.clone(),
        directory: Some(temp.path().to_path_buf()),
        jobs: None,
        verbose: false,
        command: Some(Commands::Build(BuildArgs {
            emit: Some(emit_path.clone()),
            targets: vec![],
        })),
    };
    let env = DefaultEnv::new();
    let result = run(&cli, &env);
    assert!(result.is_ok());
    assert!(emit_path.exists());
    assert!(nested_dir.exists());

    // Drop the fake ninja artifacts. PATH is restored by guard drop.
    drop(ninja_path);
}

#[test]
fn run_manifest_subcommand_writes_file() {
    let temp = tempfile::tempdir().expect("temp dir");
    let manifest_path = temp.path().join("Netsukefile");
    std::fs::copy("tests/data/minimal.yml", &manifest_path).expect("copy manifest");
    let output_path = temp.path().join("standalone.ninja");
    let cli = Cli {
        file: manifest_path.clone(),
        directory: Some(temp.path().to_path_buf()),
        jobs: None,
        verbose: false,
        command: Some(Commands::Manifest {
            file: output_path.clone(),
        }),
    };
    let env = DefaultEnv::new();
    let result = run(&cli, &env);
    assert!(result.is_ok());
    assert!(output_path.exists());
    assert!(!temp.path().join("build.ninja").exists());
}

#[test]
#[serial]
fn run_respects_env_override_for_ninja() {
    let (temp_dir, ninja_path) = support::fake_ninja(0);
    let original = std::env::var_os(NINJA_ENV);
    // SAFETY: Rust 2024 marks `set_var` as unsafe. This test injects a bogus
    // value and restores `NINJA_ENV` afterwards to avoid leaking state.
    unsafe {
        std::env::set_var(NINJA_ENV, "does-not-exist");
    }

    let mut env = MockEnv::new();
    let path_string = ninja_path
        .to_str()
        .expect("ninja path is valid UTF-8")
        .to_string();
    env.expect_raw().returning(move |key| {
        assert_eq!(key, NINJA_ENV);
        Ok(path_string.clone())
    });

    let temp = tempfile::tempdir().expect("temp dir");
    let manifest_path = temp.path().join("Netsukefile");
    std::fs::copy("tests/data/minimal.yml", &manifest_path).expect("copy manifest");
    let cli = Cli {
        file: manifest_path.clone(),
        directory: Some(temp.path().to_path_buf()),
        jobs: None,
        verbose: false,
        command: Some(Commands::Build(BuildArgs {
            emit: None,
            targets: vec![],
        })),
    };

    let result = run(&cli, &env);
    assert!(result.is_ok());

    // SAFETY: restore original `NINJA_ENV` for other tests.
    unsafe {
        if let Some(val) = original {
            std::env::set_var(NINJA_ENV, val);
        } else {
            std::env::remove_var(NINJA_ENV);
        }
    }
    drop(ninja_path);
    drop(temp_dir);
}
