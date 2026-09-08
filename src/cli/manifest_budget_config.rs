//! Defines manifest-budget configuration defaults and validation.
//!
//! Keeping this policy slice separate lets the generated build-script CLI
//! schema remain within the repository's module-size boundary while retaining
//! one authoritative source for the values exposed by CLI configuration.

use super::{CliConfig, validation_error};
use ortho_config::OrthoResult;

/// Safe default `MiniJinja` instructions allocated to one evaluation.
pub(super) const DEFAULT_MANIFEST_EVALUATION_FUEL: u64 = 1_000_000;
/// Safe default instructions allocated to one manifest.
pub(super) const DEFAULT_MANIFEST_FUEL: u64 = 100_000_000;
/// Safe default bytes emitted by one rendered value.
pub(super) const DEFAULT_MANIFEST_RENDERED_VALUE_BYTES: usize = 1_048_576;
/// Safe default aggregate rendered bytes per manifest.
pub(super) const DEFAULT_MANIFEST_RENDERED_BYTES: usize = 16_777_216;
/// Safe default source bytes consumed per manifest.
pub(super) const DEFAULT_MANIFEST_SOURCE_BYTES: usize = 4_194_304;
/// Safe default `foreach` cardinality.
pub(super) const DEFAULT_MANIFEST_FOREACH_CARDINALITY: usize = 10_000;
/// Safe default aggregate expansion count.
pub(super) const DEFAULT_MANIFEST_EXPANDED_ENTRIES: usize = 50_000;

/// Validate that all merged manifest-budget limits remain positive.
///
/// # Errors
///
/// Returns a validation error when any resource ceiling is zero.
pub(super) fn validate_manifest_budget(config: &CliConfig) -> OrthoResult<()> {
    let valid = config.manifest_evaluation_fuel > 0
        && config.manifest_fuel > 0
        && config.manifest_rendered_value_bytes > 0
        && config.manifest_rendered_manifest_bytes > 0
        && config.manifest_source_bytes > 0
        && config.manifest_foreach_cardinality > 0
        && config.manifest_expanded_entries > 0;
    if valid {
        Ok(())
    } else {
        Err(validation_error(
            "manifest budget",
            "all manifest budget limits must be positive",
        ))
    }
}
