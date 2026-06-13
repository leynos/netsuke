//! File creation helpers for the Ninja runner.
//! Handles temporary build files and writes to capability-based directories.

use crate::localization::{self, keys};
use crate::runner::NinjaContent;
use anyhow::{Context, Result as AnyResult, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs as cap_fs};
use std::io;
use std::io::Write;
use std::path::Path;
use tempfile::{Builder, NamedTempFile};
use tracing::info;

/// Return `true` when `path` is the CLI sentinel indicating "write to stdout".
#[must_use]
pub fn is_stdout_path(path: &Path) -> bool {
    path.as_os_str() == "-"
}

/// Write `content` to a freshly created temporary `*.ninja` file.
///
/// The returned [`NamedTempFile`] keeps the file alive; it is deleted when the
/// guard is dropped, so callers must retain it for as long as the build file
/// is needed. The contents are flushed and `fsync`ed before returning so a
/// spawned `ninja` reads a complete file.
///
/// # Errors
///
/// Returns an error if the temporary file cannot be created, written, flushed,
/// or synced to disk.
pub fn create_temp_ninja_file(content: &NinjaContent) -> AnyResult<NamedTempFile> {
    let mut tmp = Builder::new()
        .prefix("netsuke.")
        .suffix(".ninja")
        .tempfile()
        .context(localization::message(keys::RUNNER_IO_CREATE_TEMP_FILE))?;
    {
        let handle = tmp.as_file_mut();
        handle
            .write_all(content.as_str().as_bytes())
            .context(localization::message(keys::RUNNER_IO_WRITE_TEMP_NINJA))?;
        handle
            .flush()
            .context(localization::message(keys::RUNNER_IO_FLUSH_TEMP_NINJA))?;
        handle
            .sync_all()
            .context(localization::message(keys::RUNNER_IO_SYNC_TEMP_NINJA))?;
    }
    info!("Wrote temporary Ninja file to {}", tmp.path().display());
    Ok(tmp)
}

/// Write `content` to `path`, relative to the capability-scoped `dir`.
///
/// Any missing parent directories under `dir` are created first. The file is
/// flushed and `fsync`ed before returning. Using a [`cap_std`](cap_fs) handle
/// confines the write to the directory tree rooted at `dir`.
///
/// # Errors
///
/// Returns an error if a parent directory cannot be created, or if the file
/// cannot be created, written, flushed, or synced.
pub fn write_text_file_utf8(dir: &cap_fs::Dir, path: &Utf8Path, content: &str) -> AnyResult<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_str().is_empty()) {
        dir.create_dir_all(parent.as_str()).with_context(|| {
            localization::message(keys::RUNNER_IO_CREATE_PARENT_DIR)
                .with_arg("path", parent.as_str())
        })?;
    }
    let mut file = dir.create(path.as_str()).with_context(|| {
        localization::message(keys::RUNNER_IO_CREATE_NINJA_FILE).with_arg("path", path.as_str())
    })?;
    file.write_all(content.as_bytes()).with_context(|| {
        localization::message(keys::RUNNER_IO_WRITE_NINJA_FILE).with_arg("path", path.as_str())
    })?;
    file.flush().with_context(|| {
        localization::message(keys::RUNNER_IO_FLUSH_NINJA_FILE).with_arg("path", path.as_str())
    })?;
    file.sync_all().with_context(|| {
        localization::message(keys::RUNNER_IO_SYNC_NINJA_FILE).with_arg("path", path.as_str())
    })?;
    Ok(())
}

fn derive_dir_and_relative(path: &Utf8Path) -> AnyResult<(cap_fs::Dir, Utf8PathBuf)> {
    if path.is_relative() {
        let dir = cap_fs::Dir::open_ambient_dir(".", ambient_authority())
            .context(localization::message(keys::RUNNER_IO_OPEN_AMBIENT_DIR))?;
        return Ok((dir, path.to_owned()));
    }

    let mut ancestors = path.ancestors();
    ancestors.next();
    let (base, dir) = ancestors
        .find_map(|candidate| {
            cap_fs::Dir::open_ambient_dir(candidate.as_str(), ambient_authority())
                .ok()
                .map(|dir| (candidate.to_owned(), dir))
        })
        .ok_or_else(|| {
            anyhow!(
                localization::message(keys::RUNNER_IO_NO_EXISTING_ANCESTOR)
                    .with_arg("path", path.as_str())
                    .to_string()
            )
        })?;
    let relative = path
        .strip_prefix(&base)
        .context(localization::message(keys::RUNNER_IO_DERIVE_RELATIVE_PATH))?
        .to_owned();
    Ok((dir, relative))
}

/// Write generated Ninja `content` to `path`.
///
/// Thin wrapper over [`write_text_file`] that unwraps the [`NinjaContent`]
/// newtype.
///
/// # Errors
///
/// Returns an error if `path` is not valid UTF-8, if no existing ancestor
/// directory can be opened, or if the write fails.
pub fn write_ninja_file(path: &Path, content: &NinjaContent) -> AnyResult<()> {
    write_text_file(path, content.as_str())?;
    Ok(())
}

/// Write `content` to `path`, resolving it against a capability-scoped root.
///
/// The path is split into the deepest existing ancestor directory (opened as a
/// [`cap_std`](cap_fs) handle) and the remaining relative path, so the write is
/// confined to that directory tree. Relative paths resolve against the current
/// working directory.
///
/// # Errors
///
/// Returns an error if `path` is not valid UTF-8, if no existing ancestor
/// directory can be opened, or if [`write_text_file_utf8`] fails.
pub fn write_text_file(path: &Path, content: &str) -> AnyResult<()> {
    let utf8_path = Utf8Path::from_path(path).ok_or_else(|| {
        anyhow!(
            localization::message(keys::RUNNER_IO_NON_UTF8_PATH)
                .with_arg("path", path.display().to_string())
                .to_string()
        )
    })?;
    let (dir, relative) = derive_dir_and_relative(utf8_path)?;
    write_text_file_utf8(&dir, &relative, content)?;
    info!("Wrote file to {utf8_path}");
    Ok(())
}

fn is_broken_pipe(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::BrokenPipe
}

fn write_all_ignoring_broken_pipe(writer: &mut impl Write, buf: &[u8]) -> io::Result<()> {
    match writer.write_all(buf) {
        Ok(()) => Ok(()),
        Err(err) if is_broken_pipe(&err) => Ok(()),
        Err(err) => Err(err),
    }
}

fn flush_ignoring_broken_pipe(writer: &mut impl Write) -> io::Result<()> {
    match writer.flush() {
        Ok(()) => Ok(()),
        Err(err) if is_broken_pipe(&err) => Ok(()),
        Err(err) => Err(err),
    }
}

/// Write generated Ninja `content` to standard output.
///
/// Thin wrapper over [`write_text_stdout`] that unwraps the [`NinjaContent`]
/// newtype, used for the `-` stdout sentinel.
///
/// # Errors
///
/// Returns an error if writing to or flushing standard output fails for a
/// reason other than a broken pipe (a closed downstream reader is treated as
/// success).
pub fn write_ninja_stdout(content: &NinjaContent) -> AnyResult<()> {
    write_text_stdout(content.as_str())
}

/// Write `content` to standard output, tolerating a closed downstream pipe.
///
/// A `BrokenPipe` error (for example when piping into `head`) is treated as
/// success so the runner exits cleanly rather than reporting spurious I/O
/// failure.
///
/// # Errors
///
/// Returns an error if writing to or flushing standard output fails for any
/// reason other than a broken pipe.
pub fn write_text_stdout(content: &str) -> AnyResult<()> {
    let mut stdout = io::stdout().lock();
    write_all_ignoring_broken_pipe(&mut stdout, content.as_bytes())
        .context(localization::message(keys::RUNNER_IO_WRITE_STDOUT))?;
    flush_ignoring_broken_pipe(&mut stdout)
        .context(localization::message(keys::RUNNER_IO_FLUSH_STDOUT))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::NinjaContent;
    use anyhow::{Context, Result, ensure};
    use camino::Utf8PathBuf;
    use cap_std::{ambient_authority, fs as cap_fs};
    use rstest::rstest;
    use std::io::{Read, Seek, SeekFrom};

    #[test]
    fn create_temp_ninja_file_supports_reopen() -> Result<()> {
        let content = NinjaContent::new(String::from("rule cc"));
        let file = create_temp_ninja_file(&content)?;

        let mut reopened = file.reopen().context("reopen temp file")?;
        let mut written = String::new();
        reopened
            .read_to_string(&mut written)
            .context("read reopened temp file")?;
        ensure!(
            written == content.as_str(),
            "reopened file contents '{written}' did not match '{expected}'",
            expected = content.as_str()
        );

        let metadata = std::fs::metadata(file.path()).context("query temp file metadata")?;
        ensure!(
            metadata.len() == content.as_str().len() as u64,
            "expected size {} but observed {}",
            content.as_str().len(),
            metadata.len()
        );
        let temp_display = file.path().display().to_string();
        let has_ninja_ext = file
            .path()
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ninja"));
        ensure!(
            has_ninja_ext,
            "temporary path should end with .ninja: {temp_display}"
        );

        reopened
            .seek(SeekFrom::Start(0))
            .context("rewind reopened temp file")?;
        written.clear();
        reopened
            .read_to_string(&mut written)
            .context("re-read reopened temp file")?;
        ensure!(
            written == content.as_str(),
            "re-read file contents '{written}' did not match '{expected}'",
            expected = content.as_str()
        );
        Ok(())
    }

    #[rstest]
    #[case("-", true)]
    #[case("out.ninja", false)]
    #[case("./-", false)]
    fn is_stdout_path_detects_dash(#[case] candidate: &str, #[case] expected: bool) {
        let path = Path::new(candidate);
        assert_eq!(
            is_stdout_path(path),
            expected,
            "unexpected result for {candidate}"
        );
    }

    #[test]
    fn write_text_file_utf8_creates_parent_directories() -> Result<()> {
        let temp = tempfile::tempdir().context("create temp dir")?;
        let dir = cap_fs::Dir::open_ambient_dir(temp.path(), ambient_authority())
            .context("open temp dir")?;
        let nested = Utf8PathBuf::from("nested/build.ninja");
        let content = "build all: phony";

        write_text_file_utf8(&dir, &nested, content)?;

        let nested_path = temp.path().join("nested").join("build.ninja");
        let written = std::fs::read_to_string(&nested_path).context("read nested file")?;
        ensure!(
            written == content,
            "nested file contents '{written}' did not match '{content}'"
        );
        let parent = nested_path.parent().context("determine parent path")?;
        ensure!(
            parent.exists(),
            "expected parent directory {} to exist",
            parent.display()
        );
        Ok(())
    }

    #[test]
    fn write_ninja_file_handles_absolute_paths() -> Result<()> {
        let temp = tempfile::tempdir().context("create temp dir")?;
        let nested = temp.path().join("nested").join("build.ninja");
        let content = NinjaContent::new(String::from("build all: phony"));

        write_ninja_file(&nested, &content)?;

        let written = std::fs::read_to_string(&nested).context("read nested file")?;
        ensure!(
            written == content.as_str(),
            "absolute path file contents '{written}' did not match '{expected}'",
            expected = content.as_str()
        );
        let parent = nested.parent().context("determine parent path")?;
        ensure!(
            parent.exists(),
            "expected parent directory {} to exist",
            parent.display()
        );
        Ok(())
    }
}
