use netsuke::cli::{BuildArgs, Cli, Commands};
use netsuke::runner::{BuildTargets, NINJA_ENV, run, run_ninja};
use rstest::rstest;
use serial_test::serial;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

mod support;
use support::ScopedEnv;

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
    let (ninja_dir, ninja_path) = support::fake_ninja_check_build_file();
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths: Vec<_> = std::env::split_paths(&original_path).collect();
    paths.insert(0, ninja_dir.path().to_path_buf());
    let new_path = std::env::join_paths(paths).expect("join paths");
    let _guard = ScopedEnv::set("PATH", &new_path);

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

    // Ensure no ninja file remains in project directory
    assert!(!temp.path().join("build.ninja").exists());

    drop(ninja_path);
}

#[cfg(unix)]
#[test]
#[serial]
fn run_build_with_emit_keeps_file() {
    let (ninja_dir, ninja_path) = support::fake_ninja_check_build_file();
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths: Vec<_> = std::env::split_paths(&original_path).collect();
    paths.insert(0, ninja_dir.path().to_path_buf());
    let new_path = std::env::join_paths(paths).expect("join paths");
    let _guard = ScopedEnv::set("PATH", &new_path);

    let temp = tempfile::tempdir().expect("temp dir");
    let manifest_path = temp.path().join("Netsukefile");
    std::fs::copy("tests/data/minimal.yml", &manifest_path).expect("copy manifest");
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

    drop(ninja_path);
}

#[cfg(unix)]
#[test]
#[serial]
fn run_build_with_emit_creates_parent_dirs() {
    let (ninja_dir, ninja_path) = support::fake_ninja(0);
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths: Vec<_> = std::env::split_paths(&original_path).collect();
    paths.insert(0, ninja_dir.path().to_path_buf());
    let new_path = std::env::join_paths(paths).expect("join paths");
    let _guard = ScopedEnv::set("PATH", &new_path);

    let temp = tempfile::tempdir().expect("temp dir");
    let manifest_path = temp.path().join("Netsukefile");
    std::fs::copy("tests/data/minimal.yml", &manifest_path).expect("copy manifest");
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

    drop(ninja_path);
}

#[test]
#[serial]
fn run_manifest_subcommand_writes_file() {
    let _guard = ScopedEnv::set("PATH", OsStr::new(""));

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
    let (temp_dir, ninja_path) = support::fake_ninja(0);
    let original = std::env::var_os(NINJA_ENV);
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
