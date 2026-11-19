//! Path resolution helpers for the runner module.
//!
//! Centralises manifest, output, and working-directory logic so the main
//! runner module stays focused on command dispatch.

use crate::cli::Cli;
use crate::localization::{self, keys};
use crate::status::{PipelineStage, StatusReporter, report_pipeline_stage};
use anyhow::{Context, Result, anyhow};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs_utf8::Dir};
use std::borrow::Cow;
use std::io::{self, ErrorKind};
use std::path::Path;

use super::RunnerError;

/// Determine the manifest path respecting the CLI's directory option.
///
/// # Errors
/// Returns an error when the resulting manifest path has no file name.
///
/// # Examples
/// ```ignore
/// use crate::cli::Cli;
/// use crate::runner::resolve_manifest_path;
/// let cli = Cli::default();
/// let path = resolve_manifest_path(&cli).expect("valid manifest path");
/// assert!(path.as_str().ends_with("Netsukefile"));
/// ```
pub(super) fn resolve_manifest_path(cli: &Cli) -> Result<Utf8PathBuf> {
    let resolved = cli
        .directory
        .as_ref()
        .map_or_else(|| cli.file.clone(), |dir| dir.join(&cli.file));
    if resolved.file_name().is_none() {
        return Err(anyhow!(
            "{}",
            localization::message(keys::RUNNER_MANIFEST_PATH_MISSING_NAME)
                .with_arg("path", resolved.as_str())
        ));
    }
    Ok(resolved)
}

/// Resolve an output path relative to the CLI working directory.
///
/// The Netsuke `-C/--directory` option behaves like a working directory change
/// for any filesystem paths supplied on the command line. When `path` is
/// relative and a directory has been configured, the returned path is
/// `directory/path`.
#[must_use]
pub(super) fn resolve_output_path<'a>(cli: &Cli, path: &'a Path) -> Cow<'a, Path> {
    if path.is_relative() {
        cli.directory.as_ref().map_or(Cow::Borrowed(path), |dir| {
            Cow::Owned(dir.as_std_path().join(path))
        })
    } else {
        Cow::Borrowed(path)
    }
}

/// Return the working directory `-C/--directory` selects.
///
/// `-C/--directory` behaves like a working directory change for the child
/// process, so every command that inspects or clears generated build state
/// resolves paths against this directory rather than the caller's process
/// working directory. An unset option keeps the process working directory.
fn selected_directory(cli: &Cli) -> Utf8PathBuf {
    cli.directory
        .clone()
        .unwrap_or_else(|| Utf8PathBuf::from("."))
}

/// Open the working directory Ninja will run in, through a capability handle.
///
/// # Errors
///
/// Returns an error when the directory cannot be opened.
pub(super) fn open_effective_dir(cli: &Cli) -> Result<Dir> {
    let selected = selected_directory(cli);
    Dir::open_ambient_dir(&selected, ambient_authority()).with_context(|| {
        localization::message(keys::RUNNER_IO_OPEN_AMBIENT_DIR).with_arg("path", selected.as_str())
    })
}

/// Resolve the working directory to an absolute path with links resolved.
///
/// A capability handle cannot resolve a path above its own root, so a check
/// that must judge a `..` spelling against the workspace needs the absolute
/// base instead. Reading this path has no effect of its own: callers use it
/// only to compare spellings, and any actual access still goes through the
/// handle from [`open_effective_dir`].
///
/// # Errors
///
/// Returns an error when the directory does not exist, cannot be read, or
/// resolves to a path that is not valid UTF-8.
pub(super) fn canonical_working_directory(cli: &Cli) -> Result<Utf8PathBuf> {
    let selected = selected_directory(cli);
    selected.canonicalize_utf8().with_context(|| {
        localization::message(keys::RUNNER_IO_OPEN_AMBIENT_DIR).with_arg("path", selected.as_str())
    })
}

/// Verify the selected manifest exists, reporting `PipelineStage::ManifestIngestion`
/// when the manifest is not found.
///
/// # Errors
///
/// Returns an error when the manifest cannot be read or does not exist; the
/// ingestion stage is reported only for a not-found manifest.
pub(super) fn ensure_manifest_exists_or_error(
    cli: &Cli,
    reporter: &dyn StatusReporter,
    manifest_path: &Utf8PathBuf,
) -> Result<()> {
    let result = ensure_manifest_exists(cli, manifest_path);
    if result
        .as_ref()
        .err()
        .and_then(|error| error.downcast_ref::<RunnerError>())
        .is_some_and(|error| matches!(error, RunnerError::ManifestNotFound { .. }))
    {
        report_pipeline_stage(reporter, PipelineStage::ManifestIngestion, None);
    }
    result
}

/// Verify the selected manifest exists without emitting command status.
///
/// Commands that need to separate a pure manifest query from status reporting
/// reuse this check, while the normal build path retains its ingestion report.
pub(super) fn ensure_manifest_exists(cli: &Cli, manifest_path: &Utf8PathBuf) -> Result<()> {
    match manifest_metadata(manifest_path) {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() != ErrorKind::NotFound => {
            return Err(error)
                .with_context(|| format!("inspect manifest metadata at {manifest_path}"));
        }
        Err(_) => {}
    }

    // `resolve_manifest_path()` validates that `file_name()` is Some.
    let manifest_name = manifest_path
        .file_name()
        .ok_or_else(|| {
            anyhow!(
                "{}",
                localization::message(keys::RUNNER_MANIFEST_PATH_MISSING_NAME)
                    .with_arg("path", manifest_path.as_str())
            )
        })?
        .to_owned();
    let directory = if cli.directory.is_some() {
        let parent = manifest_path
            .parent()
            .map_or_else(|| manifest_path.as_str(), camino::Utf8Path::as_str);
        localization::message(keys::RUNNER_MANIFEST_DIRECTORY)
            .with_arg("directory", parent)
            .to_string()
    } else {
        localization::message(keys::RUNNER_MANIFEST_CURRENT_DIRECTORY).to_string()
    };
    let message = localization::message(keys::RUNNER_MANIFEST_NOT_FOUND)
        .with_arg("manifest_name", manifest_name.as_str())
        .with_arg("directory", &directory);
    Err(RunnerError::ManifestNotFound {
        manifest_name,
        directory,
        path: manifest_path.to_path_buf().into_std_path_buf(),
        message,
        help: localization::message(keys::RUNNER_MANIFEST_NOT_FOUND_HELP),
    }
    .into())
}

/// Inspect the selected manifest through a capability-scoped directory handle.
///
/// The explicit metadata result preserves permission and other I/O failures;
/// callers may map only a genuine missing path to the user-facing diagnostic.
fn manifest_metadata(manifest_path: &Utf8Path) -> io::Result<()> {
    let parent = manifest_path
        .parent()
        .filter(|path| !path.as_str().is_empty())
        .unwrap_or_else(|| Utf8Path::new("."));
    let directory = Dir::open_ambient_dir(parent, ambient_authority())?;
    let name = manifest_path.file_name().ok_or_else(|| {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!("manifest path {manifest_path} has no file name"),
        )
    })?;
    directory.metadata(Utf8Path::new(name))?;
    Ok(())
}
