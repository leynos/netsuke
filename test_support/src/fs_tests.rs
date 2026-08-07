//! Unit tests for the ambient filesystem helpers: directory creation at
//! existing-path boundaries and the `try_is_file` contract.
//!
//! Split from `fs.rs` to keep that module within the Whitaker
//! `module_max_lines` cap; included from there via `#[path]` so the tests
//! stay a child module of `fs`.

use super::{create_dir_all, try_is_file, write};
use rstest::{fixture, rstest};
use std::io;

/// Temporary workspace for the filesystem helper tests.
///
/// The fixture returns a `Result` so tests propagate setup failures with `?`
/// instead of panicking; the `TempDir` keeps the directory alive for the
/// duration of the test body, and each test invocation gets its own.
type TempDir = io::Result<tempfile::TempDir>;

#[fixture]
fn temp_dir() -> TempDir {
    tempfile::tempdir()
}

#[rstest]
fn try_is_file_reports_a_regular_file_as_a_file(temp_dir: TempDir) -> anyhow::Result<()> {
    let temp = temp_dir?;
    let file = temp.path().join("regular-file");
    write(&file, b"fixture")?;

    anyhow::ensure!(
        try_is_file(&file)?,
        "a regular file should be reported as a file"
    );
    Ok(())
}

#[rstest]
fn try_is_file_reports_an_absent_path_as_not_a_file(temp_dir: TempDir) -> anyhow::Result<()> {
    let temp = temp_dir?;

    anyhow::ensure!(
        !try_is_file(temp.path().join("absent"))?,
        "an absent path should fold NotFound into false"
    );
    Ok(())
}

#[rstest]
fn try_is_file_reports_a_directory_as_not_a_file(temp_dir: TempDir) -> anyhow::Result<()> {
    let temp = temp_dir?;

    anyhow::ensure!(
        !try_is_file(temp.path())?,
        "a directory should not be reported as a regular file"
    );
    Ok(())
}

#[rstest]
fn try_is_file_propagates_errors_other_than_not_found(temp_dir: TempDir) -> anyhow::Result<()> {
    let temp = temp_dir?;
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

#[rstest]
fn create_dir_all_accepts_an_existing_directory(temp_dir: TempDir) -> io::Result<()> {
    let temp = temp_dir?;
    create_dir_all(temp.path())
}

#[rstest]
fn create_dir_all_rejects_an_existing_file(temp_dir: TempDir) -> anyhow::Result<()> {
    let temp = temp_dir?;
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

#[rstest]
fn create_dir_all_creates_missing_parent_directories(temp_dir: TempDir) -> anyhow::Result<()> {
    let temp = temp_dir?;
    let nested = temp.path().join("one").join("two");

    create_dir_all(&nested)?;

    anyhow::ensure!(
        nested.is_dir(),
        "recursive creation should produce a directory"
    );
    Ok(())
}
