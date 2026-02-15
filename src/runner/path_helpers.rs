//! Path resolution helpers for the runner module.
//!
//! Centralises manifest and output path logic so the main runner module stays
//! focused on command dispatch.

use crate::cli::Cli;
use crate::localization::{self, keys};
use crate::status::{PipelineStage, StatusReporter, report_pipeline_stage};
use anyhow::{Result, anyhow};
use camino::Utf8PathBuf;
use std::borrow::Cow;
use std::path::Path;

use super::RunnerError;

/// Determine the manifest path respecting the CLI's directory option.
///
/// # Errors
/// Returns an error when the CLI `file` or `directory` paths are not valid UTF-8.
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
    let file = Utf8PathBuf::from_path_buf(cli.file.clone()).map_err(|path| {
        anyhow!(
            "{}",
            localization::message(keys::RUNNER_MANIFEST_PATH_UTF8)
                .with_arg("path", path.display().to_string())
        )
    })?;
    let resolved = if let Some(dir) = &cli.directory {
        let base = Utf8PathBuf::from_path_buf(dir.clone()).map_err(|path| {
            anyhow!(
                "{}",
                localization::message(keys::RUNNER_MANIFEST_DIR_UTF8)
                    .with_arg("path", path.display().to_string())
            )
        })?;
        base.join(&file)
    } else {
        file
    };
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
        cli.directory
            .as_ref()
            .map_or_else(|| Cow::Borrowed(path), |dir| Cow::Owned(dir.join(path)))
    } else {
        Cow::Borrowed(path)
    }
}

pub(super) fn ensure_manifest_exists_or_error(
    cli: &Cli,
    reporter: &dyn StatusReporter,
    manifest_path: &Utf8PathBuf,
) -> Result<()> {
    if manifest_path.as_std_path().exists() {
        return Ok(());
    }

    report_pipeline_stage(reporter, PipelineStage::ManifestIngestion, None);
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
