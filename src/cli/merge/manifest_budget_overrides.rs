//! Collect explicit manifest-budget values for the CLI configuration layer.

use super::maybe_insert_explicit;
use crate::cli::command::Cli;
use clap::ArgMatches;
use ortho_config::OrthoResult;
use serde_json::{Map, Value};

/// Insert explicit manifest-budget ceilings into the CLI merge layer.
///
/// # Errors
///
/// Returns a validation error when a supplied value cannot be serialized.
pub(super) fn insert_manifest_budget_cli_overrides(
    cli: &Cli,
    matches: &ArgMatches,
    root: &mut Map<String, Value>,
) -> OrthoResult<()> {
    maybe_insert_explicit(
        matches,
        "manifest_evaluation_fuel",
        &cli.manifest_evaluation_fuel,
        root,
    )?;
    maybe_insert_explicit(matches, "manifest_fuel", &cli.manifest_fuel, root)?;
    maybe_insert_explicit(
        matches,
        "manifest_rendered_value_bytes",
        &cli.manifest_rendered_value_bytes,
        root,
    )?;
    maybe_insert_explicit(
        matches,
        "manifest_rendered_manifest_bytes",
        &cli.manifest_rendered_manifest_bytes,
        root,
    )?;
    maybe_insert_explicit(
        matches,
        "manifest_source_bytes",
        &cli.manifest_source_bytes,
        root,
    )?;
    maybe_insert_explicit(
        matches,
        "manifest_foreach_cardinality",
        &cli.manifest_foreach_cardinality,
        root,
    )?;
    maybe_insert_explicit(
        matches,
        "manifest_expanded_entries",
        &cli.manifest_expanded_entries,
        root,
    )
}
