//! File creation helpers for the Ninja runner.
//! Handles temporary build files and writes to capability-based directories.

use crate::runner::NinjaContent;
use anyhow::{Context, Result as AnyResult, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs as cap_fs};
use std::io::Write;
use std::path::Path;
use tempfile::{Builder, NamedTempFile};
use tracing::info;

pub fn create_temp_ninja_file(content: &NinjaContent) -> AnyResult<NamedTempFile> {
    let mut tmp = Builder::new()
        .prefix("netsuke.")
        .suffix(".ninja")
        .tempfile()
        .context("create temp file")?;
    {
        let handle = tmp.as_file_mut();
        handle
            .write_all(content.as_str().as_bytes())
            .context("write temp ninja file")?;
        handle.flush().context("flush temp ninja file")?;
        handle.sync_all().context("sync temp ninja file")?;
    }
    info!("Generated temporary Ninja file at {}", tmp.path().display());
    Ok(tmp)
}

pub fn write_ninja_file_utf8(
    dir: &cap_fs::Dir,
    path: &Utf8Path,
    content: &NinjaContent,
) -> AnyResult<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_str().is_empty()) {
        dir.create_dir_all(parent.as_str())
            .with_context(|| format!("failed to create parent directory {parent}"))?;
    }
    let mut file = dir
        .create(path.as_str())
        .with_context(|| format!("failed to create Ninja file at {path}"))?;
    file.write_all(content.as_str().as_bytes())
        .with_context(|| format!("failed to write Ninja file to {path}"))?;
    file.flush()
        .with_context(|| format!("failed to flush Ninja file at {path}"))?;
    file.sync_all()
        .with_context(|| format!("failed to sync Ninja file at {path}"))?;
    Ok(())
}

fn derive_dir_and_relative(path: &Utf8Path) -> AnyResult<(cap_fs::Dir, Utf8PathBuf)> {
    if path.is_relative() {
        let dir = cap_fs::Dir::open_ambient_dir(".", ambient_authority())
            .context("open ambient directory")?;
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
        .ok_or_else(|| anyhow!("no existing ancestor for {path}"))?;
    let relative = path
        .strip_prefix(&base)
        .context("derive relative Ninja path")?
        .to_owned();
    Ok((dir, relative))
}

pub fn write_ninja_file(path: &Path, content: &NinjaContent) -> AnyResult<()> {
    let utf8_path =
        Utf8Path::from_path(path).ok_or_else(|| anyhow!("non-UTF-8 path is not supported"))?;
    let (dir, relative) = derive_dir_and_relative(utf8_path)?;
    write_ninja_file_utf8(&dir, &relative, content)?;
    info!("Generated Ninja file at {utf8_path}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::NinjaContent;
    use camino::Utf8PathBuf;
    use cap_std::{ambient_authority, fs as cap_fs};
    use std::io::{Read, Seek, SeekFrom};

    #[test]
    fn create_temp_ninja_file_supports_reopen() {
        let content = NinjaContent::new(String::from("rule cc"));
        let file = create_temp_ninja_file(&content).expect("create temp file");

        let mut reopened = file.reopen().expect("reopen temp file");
        let mut written = String::new();
        reopened
            .read_to_string(&mut written)
            .expect("read reopened temp file");
        assert_eq!(written, content.as_str());

        let metadata = std::fs::metadata(file.path()).expect("query temp file metadata");
        assert_eq!(metadata.len(), content.as_str().len() as u64);
        assert!(file.path().to_string_lossy().ends_with(".ninja"));

        reopened
            .seek(SeekFrom::Start(0))
            .expect("rewind reopened temp file");
        written.clear();
        reopened
            .read_to_string(&mut written)
            .expect("re-read reopened temp file");
        assert_eq!(written, content.as_str());
    }

    #[test]
    fn write_ninja_file_utf8_creates_parent_directories() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let dir =
            cap_fs::Dir::open_ambient_dir(temp.path(), ambient_authority()).expect("open temp dir");
        let nested = Utf8PathBuf::from("nested/build.ninja");
        let content = NinjaContent::new(String::from("build all: phony"));

        write_ninja_file_utf8(&dir, &nested, &content).expect("write ninja file");

        let nested_path = temp.path().join("nested").join("build.ninja");
        let written = std::fs::read_to_string(&nested_path).expect("read nested file");
        assert_eq!(written, content.as_str());
        assert!(nested_path.parent().expect("parent path").exists());
    }

    #[test]
    fn write_ninja_file_handles_absolute_paths() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let nested = temp.path().join("nested").join("build.ninja");
        let content = NinjaContent::new(String::from("build all: phony"));

        write_ninja_file(&nested, &content).expect("write ninja file");

        let written = std::fs::read_to_string(&nested).expect("read nested file");
        assert_eq!(written, content.as_str());
        assert!(nested.parent().expect("parent path").exists());
    }
}
