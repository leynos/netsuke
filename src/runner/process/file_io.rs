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

pub fn write_ninja_file_utf8(
    dir: &cap_fs::Dir,
    path: &Utf8Path,
    content: &NinjaContent,
) -> AnyResult<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_str().is_empty()) {
        dir.create_dir_all(parent.as_str()).with_context(|| {
            localization::message(keys::RUNNER_IO_CREATE_PARENT_DIR)
                .with_arg("path", parent.as_str())
        })?;
    }
    let mut file = dir.create(path.as_str()).with_context(|| {
        localization::message(keys::RUNNER_IO_CREATE_NINJA_FILE).with_arg("path", path.as_str())
    })?;
    file.write_all(content.as_str().as_bytes())
        .with_context(|| {
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
                "{}",
                localization::message(keys::RUNNER_IO_NO_EXISTING_ANCESTOR)
                    .with_arg("path", path.as_str())
            )
        })?;
    let relative = path
        .strip_prefix(&base)
        .context(localization::message(keys::RUNNER_IO_DERIVE_RELATIVE_PATH))?
        .to_owned();
    Ok((dir, relative))
}

pub fn write_ninja_file(path: &Path, content: &NinjaContent) -> AnyResult<()> {
    let utf8_path = Utf8Path::from_path(path).ok_or_else(|| {
        anyhow!(
            "{}",
            localization::message(keys::RUNNER_IO_NON_UTF8_PATH)
                .with_arg("path", path.display().to_string())
        )
    })?;
    let (dir, relative) = derive_dir_and_relative(utf8_path)?;
    write_ninja_file_utf8(&dir, &relative, content)?;
    info!("Wrote Ninja file to {utf8_path}");
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

pub fn write_ninja_stdout(content: &NinjaContent) -> AnyResult<()> {
    let mut stdout = io::stdout().lock();
    write_all_ignoring_broken_pipe(&mut stdout, content.as_str().as_bytes())
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
    fn write_ninja_file_utf8_creates_parent_directories() -> Result<()> {
        let temp = tempfile::tempdir().context("create temp dir")?;
        let dir = cap_fs::Dir::open_ambient_dir(temp.path(), ambient_authority())
            .context("open temp dir")?;
        let nested = Utf8PathBuf::from("nested/build.ninja");
        let content = NinjaContent::new(String::from("build all: phony"));

        write_ninja_file_utf8(&dir, &nested, &content)?;

        let nested_path = temp.path().join("nested").join("build.ninja");
        let written = std::fs::read_to_string(&nested_path).context("read nested file")?;
        ensure!(
            written == content.as_str(),
            "nested file contents '{written}' did not match '{expected}'",
            expected = content.as_str()
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
