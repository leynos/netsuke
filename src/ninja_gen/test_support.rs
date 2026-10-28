//! Shared test constructors for Ninja generation modules.

use crate::{
    ast::{CommandIsolation, Recipe, StringOrList},
    ir::{Action, BuildGraph},
};
use anyhow::{Result, bail, ensure};

use super::{NinjaGenError, generate, generate_bundle, generate_into};

/// Construct a command action with the stable default metadata used in tests.
pub(super) const fn command_action(command: StringOrList) -> Action {
    Action {
        recipe: Recipe::Command {
            command,
            isolation: CommandIsolation::Shared,
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    }
}

/// Build one action graph with the requested metadata field populated.
pub(super) fn metadata_graph(field: &str, value: &str) -> Result<BuildGraph> {
    let mut action = command_action("true".into());
    match field {
        "description" => action.description = Some(value.into()),
        "depfile" => action.depfile = Some(value.into()),
        "deps_format" => action.deps_format = Some(value.into()),
        "pool" => action.pool = Some(value.into()),
        _ => bail!("test must use a known action metadata field: {field}"),
    }
    let mut graph = BuildGraph::default();
    graph.actions.insert("metadata".into(), action);
    Ok(graph)
}

/// Map an action metadata field name to its emitted Ninja binding key.
pub(super) fn metadata_ninja_key(field: &str) -> Result<&str> {
    Ok(match field {
        "description" => "description",
        "depfile" => "depfile",
        "deps_format" => "deps",
        "pool" => "pool",
        _ => bail!("test must use a known action metadata field: {field}"),
    })
}

/// Assert every generator entry point rejects unsafe metadata without output.
pub(super) fn assert_metadata_control_character_is_rejected(
    field: &str,
    value: &str,
) -> Result<()> {
    let graph = metadata_graph(field, value)?;
    let generate_error = generate(&graph).expect_err("unsafe metadata should not generate Ninja");
    ensure!(
        matches!(generate_error, NinjaGenError::UnsafeNinjaValue),
        "{field} should produce UnsafeNinjaValue, got {generate_error:?}"
    );

    let mut output = String::new();
    let generate_into_error = generate_into(&graph, &mut output)
        .expect_err("unsafe metadata should not write Ninja output");
    ensure!(
        matches!(generate_into_error, NinjaGenError::UnsafeNinjaValue),
        "{field} should produce UnsafeNinjaValue, got {generate_into_error:?}"
    );
    ensure!(
        output.is_empty(),
        "single-action metadata validation must not emit partial Ninja output: {output}"
    );

    let bundle_error =
        generate_bundle(&graph).expect_err("unsafe metadata should not bundle Ninja");
    ensure!(
        matches!(bundle_error, NinjaGenError::UnsafeNinjaValue),
        "{field} should produce UnsafeNinjaValue, got {bundle_error:?}"
    );
    Ok(())
}
