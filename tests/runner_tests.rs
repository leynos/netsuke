use netsuke::cli::{BuildArgs, Cli, Commands};
use netsuke::runner::{BuildTargets, NINJA_ENV, run, run_ninja};
use rstest::{fixture, rstest};
use serial_test::serial;
use std::path::{Path, PathBuf};
use test_support::{
    check_ninja,
    env::{SystemEnv, prepend_dir_to_path},
    env_lock::EnvLock,
    fake_ninja,
    path_guard::PathGuard,
};

/// Fixture: Put a fake `ninja` (that checks for a build file) on `PATH`.
///
/// In Rust 2024 `std::env::set_var` is `unsafe` because it mutates
/// process-global state. `EnvLock` serialises the mutation and the returned
/// [`PathGuard`] restores the prior value, so the risk is confined to this test.
///
/// Returns: (tempdir holding ninja, `ninja_path`, PATH guard)
#[fixture]
fn ninja_in_path() -> (tempfile::TempDir, PathBuf, PathGuard) {
    let (ninja_dir, ninja_path) = check_ninja::fake_ninja_check_build_file();
    let env = SystemEnv::new();
    let guard = prepend_dir_to_path(&env, ninja_dir.path());
    (ninja_dir, ninja_path, guard)
}

/// Fixture: Put a fake `ninja` with a specific exit code on `PATH`.
///
/// The default exit code is 0 but may be customised via `#[with(...)]`. The
/// fixture uses `EnvLock` and [`PathGuard`] to tame the `unsafe` `set_var` call,
/// mirroring [`ninja_in_path`].
///
/// Returns: (tempdir holding ninja, `ninja_path`, PATH guard)
#[fixture]
fn ninja_with_exit_code(#[default(0u8)] exit_code: u8) -> (tempfile::TempDir, PathBuf, PathGuard) {
    let (ninja_dir, ninja_path) = fake_ninja(exit_code);
    let env = SystemEnv::new();
    let guard = prepend_dir_to_path(&env, ninja_dir.path());
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

    let result = run(&cli);
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

    let result = run(&cli);
    assert!(result.is_ok());

    // Ensure no ninja file remains in project directory
    assert!(!temp.path().join("build.ninja").exists());

    // Drop the fake ninja artefacts. PATH is restored by guard drop.
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

    let result = run(&cli);
    assert!(result.is_ok());

    assert!(emit_path.exists());
    let emitted = std::fs::read_to_string(&emit_path).expect("read emitted");
    assert!(emitted.contains("rule "));
    assert!(emitted.contains("build "));
    assert!(!temp.path().join("build.ninja").exists());

    // Drop the fake ninja artefacts. PATH is restored by guard drop.
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

    let result = run(&cli);
    assert!(result.is_ok());
    assert!(emit_path.exists());
    assert!(nested_dir.exists());

    // Drop the fake ninja artefacts. PATH is restored by guard drop.
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

    let result = run(&cli);
    assert!(result.is_ok());
    assert!(output_path.exists());
    assert!(!temp.path().join("build.ninja").exists());
}

#[test]
#[serial]
fn run_respects_env_override_for_ninja() {
    let (temp_dir, ninja_path) = fake_ninja(0u8);
    let original = std::env::var_os(NINJA_ENV);
    let _lock = EnvLock::acquire();
    // SAFETY: `EnvLock` serialises access to process-global state.
    unsafe {
        std::env::set_var(NINJA_ENV, &ninja_path);
    }

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

    let result = run(&cli);
    assert!(result.is_ok());

    // SAFETY: `EnvLock` ensures exclusive access while the variable is reset.
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
