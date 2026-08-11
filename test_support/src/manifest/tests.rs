//! Unit tests for manifest fixture creation.

use super::*;
use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use proptest::{prelude::*, test_runner::TestCaseError};
use rstest::{fixture, rstest};
use std::io::{self, Write};
use tempfile::TempDir;

type TempManifestWorkspace = Result<(TempDir, Utf8PathBuf)>;

#[fixture]
fn temp_manifest_workspace() -> TempManifestWorkspace {
    let temp = TempDir::new().context("create temp dir")?;
    let temp_path = Utf8Path::from_path(temp.path())
        .ok_or_else(|| anyhow::anyhow!("temp path is not valid UTF-8"))?
        .to_owned();
    Ok((temp, temp_path))
}

#[rstest]
fn existing_directory_manifest_path_is_rejected(
    temp_manifest_workspace: TempManifestWorkspace,
) -> Result<()> {
    let (temp, temp_path) = temp_manifest_workspace?;
    let dir = temp.path().join("dir");
    fs::create_dir(&dir).context("create directory placeholder")?;

    let Err(err) = ensure_manifest_exists(&temp_path, Utf8Path::new("dir")) else {
        anyhow::bail!("existing directory should be rejected");
    };
    anyhow::ensure!(err.kind() == io::ErrorKind::IsADirectory);
    let msg = err.to_string();
    let dir_str = dir
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("dir path is not valid UTF-8"))?;
    anyhow::ensure!(msg.contains(dir_str), "message: {msg}");
    Ok(())
}

#[rstest]
fn non_directory_parent_propagates_target_inspection_error(
    temp_manifest_workspace: TempManifestWorkspace,
) -> Result<()> {
    let (temp, temp_path) = temp_manifest_workspace?;
    let parent = temp.path().join("parent");
    fs::write(&parent, b"file").context("write placeholder parent file")?;
    let manifest = parent.join("manifest.yml");

    let Err(err) = ensure_manifest_exists(&temp_path, Utf8Path::new("parent/manifest.yml")) else {
        anyhow::bail!("non-directory parent should error");
    };
    anyhow::ensure!(
        err.kind() != io::ErrorKind::NotFound,
        "target inspection error should not be treated as absence: {err}"
    );
    let msg = err.to_string();
    let manifest_str = manifest
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("manifest path is not valid UTF-8"))?;
    anyhow::ensure!(msg.contains(manifest_str), "message: {msg}");
    Ok(())
}

#[rstest]
fn creates_missing_parent_directory_and_manifest(
    temp_manifest_workspace: TempManifestWorkspace,
) -> Result<()> {
    let (_temp, temp_path) = temp_manifest_workspace?;

    let cli_file = Utf8Path::new("missing/subdir/manifest.yml");
    let expected_path = temp_path.join(cli_file);
    anyhow::ensure!(
        !fs::exists(&expected_path),
        "precondition: path should not exist"
    );

    let manifest_path =
        ensure_manifest_exists(&temp_path, cli_file).context("create manifest when missing")?;
    anyhow::ensure!(manifest_path == expected_path, "manifest path should match");
    anyhow::ensure!(fs::exists(&manifest_path), "manifest file should exist");
    anyhow::ensure!(
        fs::exists(
            manifest_path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("manifest path missing parent"))?
        ),
        "parent directory should be created"
    );

    let contents =
        fs::read_to_string(manifest_path.as_std_path()).context("read manifest contents")?;
    anyhow::ensure!(
        contents.contains("netsuke_version:"),
        "unexpected manifest contents: {contents}"
    );
    Ok(())
}

#[rstest]
fn existing_file_manifest_path_is_returned_unchanged(
    temp_manifest_workspace: TempManifestWorkspace,
) -> Result<()> {
    let (_temp, temp_path) = temp_manifest_workspace?;
    let manifest_path = temp_path.join("manifest.yml");
    let existing_contents = b"existing manifest contents";
    fs::write(manifest_path.as_std_path(), existing_contents)
        .context("create existing manifest")?;

    let returned_path = ensure_manifest_exists(&temp_path, Utf8Path::new("manifest.yml"))
        .context("return existing manifest")?;

    anyhow::ensure!(returned_path == manifest_path, "manifest path should match");
    let contents = fs::read(manifest_path.as_std_path()).context("read existing manifest")?;
    anyhow::ensure!(contents == existing_contents, "manifest contents changed");
    Ok(())
}

#[rstest]
fn raced_file_manifest_path_is_returned_unchanged(
    temp_manifest_workspace: TempManifestWorkspace,
) -> Result<()> {
    let (_temp, temp_path) = temp_manifest_workspace?;
    let expected_path = temp_path.join("manifest.yml");
    let competing_contents = b"competing manifest contents";
    anyhow::ensure!(
        fs::inspect_path(&expected_path)? == fs::PathState::Absent,
        "precondition: manifest path should be absent"
    );

    let _hook = install_before_persist_hook(move |file, manifest_path| {
        fs::write(manifest_path.as_std_path(), competing_contents)?;
        Ok(file)
    });
    let returned_path = ensure_manifest_exists(&temp_path, Utf8Path::new("manifest.yml"))
        .context("tolerate manifest created before persistence")?;

    anyhow::ensure!(returned_path == expected_path, "manifest path should match");
    let contents = fs::read(expected_path.as_std_path()).context("read competing manifest")?;
    anyhow::ensure!(contents == competing_contents, "manifest contents changed");
    Ok(())
}

#[rstest]
fn raced_directory_manifest_path_is_rejected(
    temp_manifest_workspace: TempManifestWorkspace,
) -> Result<()> {
    let (_temp, temp_path) = temp_manifest_workspace?;
    let expected_path = temp_path.join("manifest.yml");
    anyhow::ensure!(
        fs::inspect_path(&expected_path)? == fs::PathState::Absent,
        "precondition: manifest path should be absent"
    );

    let _hook = install_before_persist_hook(|file, manifest_path| {
        fs::create_dir(manifest_path.as_std_path())?;
        Ok(file)
    });
    let Err(err) = ensure_manifest_exists(&temp_path, Utf8Path::new("manifest.yml")) else {
        anyhow::bail!("directory created before persistence should be rejected");
    };

    anyhow::ensure!(err.kind() == io::ErrorKind::IsADirectory);
    anyhow::ensure!(
        err.to_string().contains(expected_path.as_str()),
        "message: {err}"
    );
    Ok(())
}

#[rstest]
fn before_persist_hook_does_not_escape_its_scope(
    temp_manifest_workspace: TempManifestWorkspace,
) -> Result<()> {
    let (_temp, temp_path) = temp_manifest_workspace?;
    {
        let _hook = install_before_persist_hook(|_, _| {
            Err(io::Error::other(
                "hook should be removed when its guard drops",
            ))
        });
    }

    let expected_path = temp_path.join("manifest.yml");
    let returned_path = ensure_manifest_exists(&temp_path, Utf8Path::new("manifest.yml"))
        .context("create manifest after hook scope ends")?;

    anyhow::ensure!(returned_path == expected_path, "manifest path should match");
    Ok(())
}

#[rstest]
fn persisting_manifest_tolerates_existing_file_without_overwriting(
    temp_manifest_workspace: TempManifestWorkspace,
) -> Result<()> {
    let (temp, temp_path) = temp_manifest_workspace?;
    let manifest_path = temp_path.join("manifest.yml");
    let existing_contents = b"existing manifest contents";
    fs::write(manifest_path.as_std_path(), existing_contents)
        .context("create existing manifest")?;

    let mut staged_file = NamedTempFile::new_in(temp.path()).context("stage manifest")?;
    staged_file
        .write_all(b"replacement manifest contents")
        .context("write staged manifest")?;

    persist_manifest_file(staged_file, &manifest_path)
        .context("persist staged manifest without overwriting")?;

    let contents =
        fs::read(manifest_path.as_std_path()).context("read existing manifest contents")?;
    anyhow::ensure!(
        contents == existing_contents,
        "manifest contents changed: {contents:?}"
    );
    Ok(())
}

#[rstest]
fn persisting_manifest_rejects_existing_directory(
    temp_manifest_workspace: TempManifestWorkspace,
) -> Result<()> {
    let (temp, temp_path) = temp_manifest_workspace?;
    let manifest_path = temp_path.join("manifest.yml");
    fs::create_dir(manifest_path.as_std_path()).context("create manifest directory")?;
    let staged_file = NamedTempFile::new_in(temp.path()).context("stage manifest")?;

    let Err(err) = persist_manifest_file(staged_file, &manifest_path) else {
        anyhow::bail!("directory target should be rejected");
    };
    anyhow::ensure!(err.kind() == io::ErrorKind::IsADirectory);
    anyhow::ensure!(
        err.to_string().contains(manifest_path.as_str()),
        "message: {err}"
    );
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum TargetState {
    Missing,
    ExistingFile,
    RacedFile,
    ExistingDirectory,
    RacedDirectory,
}

fn target_state_strategy() -> impl Strategy<Value = TargetState> {
    prop_oneof![
        Just(TargetState::Missing),
        Just(TargetState::ExistingFile),
        Just(TargetState::RacedFile),
        Just(TargetState::ExistingDirectory),
        Just(TargetState::RacedDirectory),
    ]
}

fn property_workspace() -> Result<(TempDir, Utf8PathBuf), TestCaseError> {
    let temp = TempDir::new().map_err(|error| TestCaseError::fail(error.to_string()))?;
    let temp_path = Utf8Path::from_path(temp.path())
        .ok_or_else(|| TestCaseError::fail("temporary path is not UTF-8"))?
        .to_owned();
    Ok((temp, temp_path))
}

fn replace_staged_manifest(
    staged_file: NamedTempFile,
    manifest_path: &Utf8Path,
    contents: &[u8],
) -> io::Result<NamedTempFile> {
    drop(staged_file);
    let destination_directory = manifest_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Manifest path has no parent directory: {manifest_path}"),
        )
    })?;
    let mut replacement = NamedTempFile::new_in(destination_directory.as_std_path())?;
    replacement.write_all(contents)?;
    Ok(replacement)
}

// This property checks controlled creation orderings through the injected hook.
// It does not model arbitrary operating-system scheduling or filesystems.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn manifest_existence_contract_holds_for_bounded_target_states(
        state in target_state_strategy(),
        staged_name in "[a-z]{1,12}",
        competing_contents in proptest::collection::vec(any::<u8>(), 1..25),
    ) {
        let (_temp, temp_path) = property_workspace()?;
        let cli_file = Utf8Path::new("manifest.yml");
        let expected_path = temp_path.join(cli_file);
        let staged_contents = manifest_yaml(&format!(
            "targets:\n  - name: {staged_name}\n    command: \"echo hi\"\n"
        ))
        .into_bytes();

        match state {
            TargetState::ExistingFile => fs::write(expected_path.as_std_path(), &competing_contents)
                .map_err(|error| TestCaseError::fail(error.to_string()))?,
            TargetState::ExistingDirectory => fs::create_dir(expected_path.as_std_path())
                .map_err(|error| TestCaseError::fail(error.to_string()))?,
            TargetState::Missing | TargetState::RacedFile | TargetState::RacedDirectory => {}
        }

        let expected_staged_contents = staged_contents.clone();
        let hook_contents = competing_contents.clone();
        let _hook = install_before_persist_hook(move |file, manifest_path| {
                let replacement = replace_staged_manifest(file, manifest_path, &staged_contents)?;
                match state {
                    TargetState::RacedFile => {
                        fs::write(manifest_path.as_std_path(), hook_contents)?;
                    }
                    TargetState::RacedDirectory => fs::create_dir(manifest_path.as_std_path())?,
                    TargetState::Missing
                    | TargetState::ExistingFile
                    | TargetState::ExistingDirectory => {}
                }
                Ok(replacement)
            });
        let result = ensure_manifest_exists(&temp_path, cli_file);

        match state {
            TargetState::Missing => {
                let returned_path = result.map_err(|error| TestCaseError::fail(error.to_string()))?;
                prop_assert_eq!(&returned_path, &expected_path);
                let contents = fs::read(expected_path.as_std_path())
                    .map_err(|error| TestCaseError::fail(error.to_string()))?;
                prop_assert_eq!(contents, expected_staged_contents);
            }
            TargetState::ExistingFile | TargetState::RacedFile => {
                let returned_path = result.map_err(|error| TestCaseError::fail(error.to_string()))?;
                prop_assert_eq!(&returned_path, &expected_path);
                let contents = fs::read(expected_path.as_std_path())
                    .map_err(|error| TestCaseError::fail(error.to_string()))?;
                prop_assert_eq!(contents, competing_contents);
            }
            TargetState::ExistingDirectory | TargetState::RacedDirectory => {
                let error = result.expect_err("directory target should be rejected");
                prop_assert_eq!(error.kind(), io::ErrorKind::IsADirectory);
                prop_assert!(error.to_string().contains(expected_path.as_str()));
            }
        }
    }
}
