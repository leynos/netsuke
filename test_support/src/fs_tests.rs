//! Unit tests for the ambient filesystem helpers: directory creation at
//! existing-path boundaries and the `try_is_file` contract.
//!
//! Split from `fs.rs` to keep that module within the Whitaker
//! `module_max_lines` cap; included from there via `#[path]` so the tests
//! stay a child module of `fs`.

use super::{create_dir_all, try_is_file, write};
use std::io;

#[test]
fn try_is_file_reports_a_regular_file_as_a_file() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let file = temp.path().join("regular-file");
    write(&file, b"fixture")?;

    anyhow::ensure!(
        try_is_file(&file)?,
        "a regular file should be reported as a file"
    );
    Ok(())
}

#[test]
fn try_is_file_reports_an_absent_path_as_not_a_file() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;

    anyhow::ensure!(
        !try_is_file(temp.path().join("absent"))?,
        "an absent path should fold NotFound into false"
    );
    Ok(())
}

#[test]
fn try_is_file_reports_a_directory_as_not_a_file() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;

    anyhow::ensure!(
        !try_is_file(temp.path())?,
        "a directory should not be reported as a regular file"
    );
    Ok(())
}

#[test]
fn try_is_file_propagates_errors_other_than_not_found() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let file = temp.path().join("regular-file");
    write(&file, b"fixture")?;

    let Err(error) = try_is_file(file.join("child")) else {
        anyhow::bail!("traversing through a regular file should fail");
    };
    anyhow::ensure!(
        error.kind() != io::ErrorKind::NotFound,
        "traversal through a file should not be reported as absence, got {error:?}"
    );
    Ok(())
}

#[test]
fn create_dir_all_accepts_an_existing_directory() -> io::Result<()> {
    let temp = tempfile::tempdir()?;
    create_dir_all(temp.path())
}

#[test]
fn create_dir_all_rejects_an_existing_file() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let file = temp.path().join("not-a-directory");
    write(&file, b"fixture")?;

    let Err(error) = create_dir_all(&file) else {
        anyhow::bail!("an existing file should not be accepted as a directory");
    };
    anyhow::ensure!(
        matches!(
            error.kind(),
            io::ErrorKind::AlreadyExists | io::ErrorKind::NotADirectory
        ),
        "existing file should report a directory conflict, got {error:?}"
    );
    Ok(())
}

#[test]
fn create_dir_all_creates_missing_parent_directories() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let nested = temp.path().join("one").join("two");

    create_dir_all(&nested)?;

    anyhow::ensure!(
        nested.is_dir(),
        "recursive creation should produce a directory"
    );
    Ok(())
}
