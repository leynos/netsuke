//! Post-merge validation for the layered CLI configuration.
//!
//! Keeping these checks in their own module lets [`super::config`] remain
//! within the repository's module-size boundary while the merged
//! [`CliConfig`] still rejects unsupported values at configuration
//! composition rather than later during runner setup.

use super::{CliConfig, NO_INPUT_VALIDATION_REASON, validation_error};
use ortho_config::OrthoResult;

/// Maximum number of parallel build jobs accepted by the CLI.
pub(super) const MAX_JOBS: usize = super::super::validation::MAX_JOBS;

/// Return whether `jobs` falls outside the accepted range.
const fn jobs_out_of_bounds(jobs: usize) -> bool {
    jobs == 0 || jobs > MAX_JOBS
}

/// Verify that the merged manifest path remains valid UTF-8.
///
/// The `Utf8PathBuf` field makes this invariant structural for file and
/// environment layers. The final validation keeps the post-merge seam
/// explicit, so any future untyped source is rejected at configuration
/// composition rather than later during runner setup.
///
/// # Errors
///
/// Returns a validation error when the merged manifest path is not UTF-8.
pub(super) fn validate_manifest_path(config: &CliConfig) -> OrthoResult<()> {
    if config.file.as_std_path().to_str().is_some() {
        Ok(())
    } else {
        Err(validation_error("file", "manifest path is not valid UTF-8"))
    }
}

/// Validate that non-interactive mode stays enabled after merging.
///
/// Netsuke has no interactive mode, so a merged `no_input = false` is
/// unsupported and rejected by the post-merge hook.
///
/// # Errors
///
/// Returns a validation error when `no_input` resolves to false.
pub(super) fn validate_non_interactive(config: &CliConfig) -> OrthoResult<()> {
    if config.no_input.is_enabled() {
        Ok(())
    } else {
        Err(validation_error("no_input", NO_INPUT_VALIDATION_REASON))
    }
}

/// Validate that the merged job count falls within the supported range.
///
/// # Errors
///
/// Returns a validation error when `jobs` is zero or greater than `MAX_JOBS`.
pub(super) fn validate_jobs(config: &CliConfig) -> OrthoResult<()> {
    let Some(jobs) = config.jobs else {
        return Ok(());
    };
    if jobs_out_of_bounds(jobs) {
        return Err(validation_error(
            "jobs",
            &format!("jobs = {jobs} is out of range; must be between 1 and {MAX_JOBS}"),
        ));
    }
    Ok(())
}
