//! Helpers for constructing manifest fixtures in tests.

use crate::fs;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8};
use std::io;
use tempfile::NamedTempFile;

/// Prefix the provided manifest body with the standard Netsuke version header.
#[must_use]
pub fn manifest_yaml(body: &str) -> String {
    format!("netsuke_version: 1.0.0\n{body}")
}

/// Resolve `cli_file` relative to `temp_dir` and ensure it exists.
///
/// When `cli_file` is relative, it is joined with `temp_dir` and the returned
/// path is absolute and UTF‑8. If the resulting path already names a
/// non-directory target, that path is returned without modifying its contents.
/// If another actor creates a non-directory target after the initial existence
/// check but before persistence, that target is likewise returned unchanged.
/// Directory targets return an [`io::ErrorKind::IsADirectory`] error. When a
/// manifest must be created, staging occurs atomically in the target directory.
/// These guarantees describe the controlled implementation ordering and do not
/// establish behaviour for every filesystem or scheduler.
///
/// # Errors
///
/// Returns an [`io::Error`] if any I/O error occurs whilst validating the
/// target, creating parent directories, writing the temporary manifest, or
/// persisting it to `manifest_path`.
///
/// # Examples
///
/// ```rust,ignore
/// use camino::{Utf8Path, Utf8PathBuf};
/// use tempfile::TempDir;
/// use test_support::ensure_manifest_exists;
///
/// let temp = TempDir::new().expect("temp dir");
/// let temp_path = Utf8Path::from_path(temp.path()).expect("utf-8 path");
/// let cli_file = Utf8PathBuf::from("manifest.yml");
/// let manifest = ensure_manifest_exists(temp_path, &cli_file)
///     .expect("manifest");
/// assert!(manifest.exists());
/// ```
pub fn ensure_manifest_exists(temp_dir: &Utf8Path, cli_file: &Utf8Path) -> io::Result<Utf8PathBuf> {
    let manifest_path = resolve_manifest_path(temp_dir, cli_file)?;

    if fs::is_dir(&manifest_path) {
        return Err(io::Error::new(
            io::ErrorKind::IsADirectory,
            format!("Manifest path points to a directory, expected a file: {manifest_path}"),
        ));
    }

    if fs::exists(&manifest_path) {
        return Ok(manifest_path);
    }

    create_manifest_file(temp_dir, manifest_path.as_ref())?;
    Ok(manifest_path)
}

fn resolve_manifest_path(temp_dir: &Utf8Path, cli_file: &Utf8Path) -> io::Result<Utf8PathBuf> {
    let manifest_path = if cli_file.is_absolute() {
        cli_file.to_owned()
    } else {
        temp_dir.join(cli_file)
    };

    if manifest_path.file_name().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Manifest path must include a file name: {manifest_path}"),
        ));
    }

    Ok(manifest_path)
}

fn create_manifest_file(temp_dir: &Utf8Path, manifest_path: &Utf8Path) -> io::Result<()> {
    let dest_dir = manifest_path.parent().unwrap_or(temp_dir);
    ensure_parent_directory(manifest_path, dest_dir)?;
    let mut file = create_temp_file(dest_dir, manifest_path)?;
    write_manifest_content(&mut file, manifest_path)?;
    persist_manifest_file(file, manifest_path)
}

fn create_temp_file(dest_dir: &Utf8Path, manifest_path: &Utf8Path) -> io::Result<NamedTempFile> {
    NamedTempFile::new_in(dest_dir.as_std_path()).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("Failed to create temporary manifest file for {manifest_path}: {e}"),
        )
    })
}

fn write_manifest_content(file: &mut NamedTempFile, manifest_path: &Utf8Path) -> io::Result<()> {
    crate::env::write_manifest(file).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("Failed to write manifest content to {manifest_path}: {e}"),
        )
    })
}

/// Persist a staged manifest without overwriting an existing target.
///
/// A concurrently created non-directory target is tolerated because it already
/// fulfils the manifest-exists contract.
fn persist_manifest_file(file: NamedTempFile, manifest_path: &Utf8Path) -> io::Result<()> {
    match file.persist_noclobber(manifest_path.as_std_path()) {
        Ok(_) => Ok(()),
        Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists && fs::is_dir(manifest_path) => {
            Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                format!("Manifest path points to a directory, expected a file: {manifest_path}"),
            ))
        }
        Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists && fs::exists(manifest_path) => {
            Ok(())
        }
        Err(e) => Err(io::Error::new(
            e.error.kind(),
            format!(
                "Failed to persist manifest file to {} from {}: {}",
                manifest_path,
                e.file.path().display(),
                e.error
            ),
        )),
    }
}

fn ensure_parent_directory(manifest_path: &Utf8Path, dest_dir: &Utf8Path) -> io::Result<()> {
    if fs::exists(dest_dir) {
        // If the path exists but is not a directory, report a clear error that
        // includes the final manifest path. Returning AlreadyExists mirrors the
        // semantics that the desired directory “exists” but is unusable.
        if fs::is_dir(dest_dir) {
            return Ok(());
        }
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "Failed to create manifest parent directory for {manifest_path}: parent path exists and is not a directory",
            ),
        ));
    }

    let base = find_existing_ancestor(dest_dir, manifest_path)?;

    let relative = dest_dir.strip_prefix(base).map_err(|_| {
        io::Error::other(format!(
            "Failed to derive relative path for {dest_dir} from ancestor {base}",
        ))
    })?;

    let dir = fs_utf8::Dir::open_ambient_dir(base, ambient_authority()).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("Failed to open ancestor directory {base} for {manifest_path}: {e}"),
        )
    })?;

    dir.create_dir_all(relative).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("Failed to create manifest parent directory for {manifest_path}: {e}"),
        )
    })
}

fn find_existing_ancestor<'a>(
    dest_dir: &'a Utf8Path,
    manifest_path: &Utf8Path,
) -> io::Result<&'a Utf8Path> {
    let mut ancestors = dest_dir.ancestors();
    ancestors.next(); // Skip self

    ancestors
        .find(|candidate| fs::exists(candidate))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Failed to locate an existing ancestor for manifest directory {manifest_path}",
                ),
            )
        })
}

#[cfg(test)]
mod tests {
    //! Unit tests for manifest fixture creation.

    use super::*;
    use anyhow::{Context, Result};
    use camino::{Utf8Path, Utf8PathBuf};
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
    fn read_only_parent_reports_target_path(
        temp_manifest_workspace: TempManifestWorkspace,
    ) -> Result<()> {
        let (temp, temp_path) = temp_manifest_workspace?;
        let parent = temp.path().join("parent");
        fs::write(&parent, b"file").context("write placeholder parent file")?;
        let manifest = parent.join("manifest.yml");

        let Err(err) = ensure_manifest_exists(&temp_path, Utf8Path::new("parent/manifest.yml"))
        else {
            anyhow::bail!("non-directory parent should error");
        };
        anyhow::ensure!(err.kind() == io::ErrorKind::AlreadyExists);
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

        // Parent directory does not exist beforehand.
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

        // Sanity check that content was written, not an empty file.
        let contents =
            fs::read_to_string(manifest_path.as_std_path()).context("read manifest contents")?;
        anyhow::ensure!(
            contents.contains("netsuke_version:"),
            "unexpected manifest contents: {contents}"
        );
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
}
