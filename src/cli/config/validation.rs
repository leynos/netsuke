//! Post-merge validation for the layered CLI configuration.
//!
//! Keeping these checks in their own module lets [the parent configuration module] remain
//! within the repository's module-size boundary while the merged
//! [`CliConfig`] still rejects unsupported values at configuration
//! composition rather than later during runner setup.

use super::budget::validate_manifest_budget;
use super::shells::{SHELLS_KEY, validate_composition};
use super::{CliConfig, NO_INPUT_VALIDATION_REASON, validation_error};
use ortho_config::{OrthoResult, PostMergeContext, PostMergeHook};

/// Maximum number of parallel build jobs accepted by the CLI.
pub(super) const MAX_JOBS: usize = super::super::validation::MAX_JOBS;

impl PostMergeHook for CliConfig {
    /// Reject a merged configuration that the runner cannot honour.
    ///
    /// Every check runs after all layers have merged and before any manifest is
    /// read, so a rejected value stops startup at configuration loading.
    fn post_merge(&mut self, _ctx: &PostMergeContext) -> OrthoResult<()> {
        validate_manifest_path(self)?;
        validate_non_interactive(self)?;
        validate_jobs(self)?;
        validate_configured_shells(self)?;
        validate_manifest_budget(self)?;
        Ok(())
    }
}

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

/// Verify the merged registry of configured shells.
///
/// Each file layer already rejected a malformed definition, so this pass owns
/// only the rules that need the whole registry: no definition may reuse a
/// built-in name, and no two positions may share a name. The merged vector is
/// validated once, so a rejection leaves no partially accepted registry.
///
/// # Errors
///
/// Returns a validation error naming the registry when it shadows a built-in
/// shell name or declares one name more than once.
pub(super) fn validate_configured_shells(config: &CliConfig) -> OrthoResult<()> {
    validate_composition(&config.shells)
        .map_err(|error| validation_error(SHELLS_KEY, &error.to_string()))
}
