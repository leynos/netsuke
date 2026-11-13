//! Unit tests for the ambient filesystem helpers: directory creation at
//! existing-path boundaries and the `try_is_file` contract.
//!
//! Split from `fs.rs` to keep that module within the Whitaker
//! `module_max_lines` cap; included from there via `#[path]` so the tests
//! stay a child module of `fs`.

use super::{PathState, create_dir_all, inspect_path, try_is_file, write};
#[cfg(windows)]
use super::{create_dir, read_to_string};
use rstest::{fixture, rstest};
use std::io;
use std::path::Path;

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

fn assert_traversal_through_file_propagates_non_not_found<T>(
    temp_dir: TempDir,
    inspect: impl FnOnce(&Path) -> io::Result<T>,
) -> anyhow::Result<()> {
    let temp = temp_dir?;
    let file = temp.path().join("regular-file");
    write(&file, b"fixture")?;

    let Err(error) = inspect(&file.join("child")) else {
        anyhow::bail!("traversing through a regular file should fail");
    };
    anyhow::ensure!(
        error.kind() != io::ErrorKind::NotFound,
        "traversal through a file should not be reported as absence, got {error:?}"
    );
    Ok(())
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
    assert_traversal_through_file_propagates_non_not_found(temp_dir, |path| try_is_file(path))
}

#[rstest]
fn inspect_path_distinguishes_absent_directory_and_non_directory(
    temp_dir: TempDir,
) -> anyhow::Result<()> {
    let temp = temp_dir?;
    let file = temp.path().join("regular-file");
    write(&file, b"fixture")?;

    anyhow::ensure!(inspect_path(temp.path())? == PathState::Directory);
    anyhow::ensure!(inspect_path(&file)? == PathState::NonDirectory);
    anyhow::ensure!(inspect_path(temp.path().join("absent"))? == PathState::Absent);
    Ok(())
}

#[rstest]
fn inspect_path_propagates_errors_other_than_not_found(temp_dir: TempDir) -> anyhow::Result<()> {
    assert_traversal_through_file_propagates_non_not_found(temp_dir, |path| inspect_path(path))
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

/// A fixture directory may legitimately contain a `%NAME%` sequence. `cmd`
/// expands `%NAME%` in the command text, so the junction helper must carry the
/// paths out of band; this guards that it does.
#[cfg(windows)]
#[rstest]
fn junction_preserves_a_literal_percent_expansion(temp_dir: TempDir) -> anyhow::Result<()> {
    let temp = temp_dir?;
    let target = temp.path().join("%USERNAME%");
    create_dir(&target)?;
    let link = temp.path().join("link");

    if !super::junction(&target, &link)? {
        eprintln!("skipping junction test: cmd.exe is unavailable");
        return Ok(());
    }

    write(target.join("probe.txt"), b"literal")?;
    anyhow::ensure!(
        super::is_dir(&link),
        "the junction must point at the literal %USERNAME% directory"
    );
    anyhow::ensure!(
        read_to_string(link.join("probe.txt"))? == "literal",
        "the junction must resolve the intended target, not the ambient expansion"
    );
    Ok(())
}
