//! Helpers for constructing manifest fixtures in tests.

use crate::fs;
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8};
use std::io;
use tempfile::NamedTempFile;

#[cfg(test)]
use std::cell::RefCell;

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
/// Target inspection is fallible: metadata errors other than `NotFound`
/// propagate to the caller.
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

    match inspect_manifest_target(&manifest_path)? {
        fs::PathState::Absent => create_manifest_file(temp_dir, manifest_path.as_ref())?,
        fs::PathState::Directory => return Err(manifest_path_is_directory_error(&manifest_path)),
        fs::PathState::NonDirectory => return Ok(manifest_path),
    }

    Ok(manifest_path)
}

fn inspect_manifest_target(manifest_path: &Utf8Path) -> io::Result<fs::PathState> {
    fs::inspect_path(manifest_path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("Failed to inspect manifest path {manifest_path}: {error}"),
        )
    })
}

fn manifest_path_is_directory_error(manifest_path: &Utf8Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::IsADirectory,
        format!("Manifest path points to a directory, expected a file: {manifest_path}"),
    )
}

/// Install a deterministic action before a staged manifest is persisted.
///
/// This test-only seam models controlled target creation after the initial
/// existence check; it does not introduce scheduling into production code. Its
/// guard restores the preceding hook when it is dropped.
#[cfg(test)]
fn install_before_persist_hook(
    hook: impl FnOnce(NamedTempFile, &Utf8Path) -> io::Result<NamedTempFile> + 'static,
) -> BeforePersistHookGuard {
    let previous = BEFORE_PERSIST_HOOK.with(|hook_slot| hook_slot.replace(Some(Box::new(hook))));
    BeforePersistHookGuard { previous }
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
    #[cfg(test)]
    {
        persist_manifest_file(run_before_persist_hook(file, manifest_path)?, manifest_path)
    }

    #[cfg(not(test))]
    {
        persist_manifest_file(file, manifest_path)
    }
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
        Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => {
            match inspect_manifest_target(manifest_path)? {
                fs::PathState::Directory => Err(manifest_path_is_directory_error(manifest_path)),
                fs::PathState::NonDirectory => Ok(()),
                fs::PathState::Absent => Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "Manifest target disappeared after no-clobber persistence reported it existed: {manifest_path}"
                    ),
                )),
            }
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
    match fs::inspect_path(dest_dir).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("Failed to inspect manifest parent directory for {manifest_path}: {error}"),
        )
    })? {
        fs::PathState::Directory => return Ok(()),
        fs::PathState::NonDirectory => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "Failed to create manifest parent directory for {manifest_path}: parent path exists and is not a directory",
                ),
            ));
        }
        fs::PathState::Absent => {}
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

    for candidate in ancestors {
        match fs::inspect_path(candidate).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "Failed to inspect manifest ancestor {candidate} for {manifest_path}: {error}"
                ),
            )
        })? {
            fs::PathState::Directory => return Ok(candidate),
            fs::PathState::NonDirectory => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "Failed to create manifest parent directory for {manifest_path}: ancestor {candidate} is not a directory",
                    ),
                ));
            }
            fs::PathState::Absent => {}
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("Failed to locate an existing ancestor for manifest directory {manifest_path}"),
    ))
}

#[cfg(test)]
type BeforePersistHook = Box<dyn FnOnce(NamedTempFile, &Utf8Path) -> io::Result<NamedTempFile>>;

#[cfg(test)]
thread_local! {
    static BEFORE_PERSIST_HOOK: RefCell<Option<BeforePersistHook>> = const { RefCell::new(None) };
}

#[cfg(test)]
#[must_use]
struct BeforePersistHookGuard {
    previous: Option<BeforePersistHook>,
}

#[cfg(test)]
impl Drop for BeforePersistHookGuard {
    fn drop(&mut self) {
        BEFORE_PERSIST_HOOK.with(|hook_slot| *hook_slot.borrow_mut() = self.previous.take());
    }
}

#[cfg(test)]
fn run_before_persist_hook(
    file: NamedTempFile,
    manifest_path: &Utf8Path,
) -> io::Result<NamedTempFile> {
    let hook = BEFORE_PERSIST_HOOK.with(|hook_slot| hook_slot.borrow_mut().take());
    match hook {
        Some(installed_hook) => installed_hook(file, manifest_path),
        None => Ok(file),
    }
}

#[cfg(test)]
#[path = "manifest/tests.rs"]
mod tests;
