//! Unit tests for command configuration and tempfile management.

use super::*;
use crate::stdlib::command::tests_support::test_command_config;
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};
use tempfile::tempdir;
use test_support::fs;

#[test]
fn sanitize_label_replaces_disallowed_characters() {
    assert_eq!(sanitize_label("std:out/..*"), "std-out----");
}

#[test]
fn command_tempfile_drop_removes_file() -> Result<()> {
    let (_temp_dir, config) = test_command_config()?;
    let temp_path = {
        let tempfile = config
            .create_tempfile("stdout")
            .context("create temp file")?;
        let path = tempfile.path().to_path_buf();
        ensure!(path.exists(), "tempfile should exist while handle is alive");
        path
    };
    ensure!(
        !temp_path.exists(),
        "temporary file should be removed on drop"
    );
    Ok(())
}

#[test]
fn command_tempfile_into_path_persists_file() -> Result<()> {
    let (_temp_dir, config) = test_command_config()?;
    let tempfile = config
        .create_tempfile("stdout")
        .context("create temp file")?;
    let expected = tempfile.path().to_path_buf();
    let kept = tempfile
        .into_temp_path()
        .keep()
        .map_err(|err| err.error)
        .context("persist temp file")?;
    ensure!(
        kept.as_path() == expected.as_path(),
        "kept path {} did not match {}",
        kept.display(),
        expected.display()
    );
    ensure!(kept.as_path().exists(), "persisted temp file should exist");
    fs::remove_file(kept.as_path()).context("cleanup persisted temp file")?;
    Ok(())
}

#[test]
fn command_tempdir_requires_workspace_root_path() {
    let temp = tempdir().expect("create temp workspace for command");
    let path = Utf8PathBuf::from_path_buf(temp.path().to_path_buf())
        .expect("temp workspace should be valid UTF-8");
    let dir = Dir::open_ambient_dir(&path, ambient_authority()).expect("open temp workspace dir");
    let config = CommandConfig::new(CommandConfigInit {
        max_capture_bytes: 1024,
        max_stream_bytes: 2048,
        workspace_root: Arc::new(dir),
        workspace_root_path: None,
        command_path_override: None,
    });
    match config.create_tempfile("stdout") {
        Ok(_) => panic!("command temp dir should require workspace root path"),
        Err(err) => assert_eq!(err.kind(), io::ErrorKind::InvalidInput),
    }
}
