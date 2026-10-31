//! Release-help metadata translated from Netsuke's Clap command.
//!
//! The parser-only selectors `-C/--directory` and `--config` exist only in the
//! Clap command: they have no configuration, environment, or file source, so
//! they cannot come from the `OrthoConfig` derive. The configuration projection
//! in the parent module also reconciles each configuration field's CLI shape
//! through the same translator, because the derive's invented flag spellings
//! and `skip_cli` omissions do not describe the binary. Keeping the translation
//! here leaves one reason to change: `src/cli/command.rs`.

use anyhow::{Context, Result, bail};
use clap::{Arg, ArgAction, Command};
use ortho_config::docs::{CliMetadata, FieldMetadata, ValueType};
use std::collections::HashSet;

use crate::cli::l10n::top_level_flag_help_key;

/// Return whether `argument` is part of the documented configuration surface.
///
/// Clap's generated `--help` and `--version` switches belong to the tooling
/// rather than to Netsuke's configuration model, so the manual page describes
/// them generically instead of listing them as options.
pub(super) fn is_documented_root_argument(argument: &Arg) -> bool {
    !matches!(argument.get_id().as_str(), "help" | "version")
}

/// Translate one Clap argument into release-help CLI metadata.
///
/// Every member is read from the parser so the generated manual page cannot
/// advertise a spelling or value shape the binary rejects. Switches report no
/// value at all, even though Clap records possible values for their `SetTrue`
/// action. `value_optional` follows the argument's arity minimum, so a
/// genuinely optional value keeps its `--flag[=VALUE]` spelling while a
/// required one does not gain it.
pub(super) fn cli_metadata_from_argument(argument: &Arg) -> CliMetadata {
    let takes_value = argument.get_action().takes_values();
    let value_name = if takes_value {
        argument
            .get_value_names()
            .and_then(|names| names.first())
            .map(ToString::to_string)
    } else {
        None
    };
    let possible_values = if takes_value {
        argument
            .get_possible_values()
            .iter()
            .map(|value| value.get_name().to_owned())
            .collect()
    } else {
        Vec::new()
    };

    CliMetadata {
        long: argument.get_long().map(str::to_owned),
        short: argument.get_short(),
        value_name,
        multiple: matches!(argument.get_action(), &ArgAction::Append),
        takes_value,
        value_optional: takes_value
            && argument
                .get_num_args()
                .is_some_and(|range| range.min_values() == 0),
        possible_values,
        hide_in_help: argument.is_hide_set(),
    }
}

/// Build release-help metadata for every parser-only top-level selector.
///
/// Configuration fields are excluded by name, so a selector that later gains a
/// configuration source is described once, by the configuration projection,
/// rather than twice. Discovery retains the selectors' own policy in
/// `discovery.rs` under ADR-0048.
///
/// # Errors
///
/// Returns an error when a parser-only selector lacks a Fluent key or path
/// metadata, which would leave generated release help incomplete.
pub(super) fn documented_clap_parser_fields(
    command: &Command,
    configuration_fields: &[FieldMetadata],
) -> Result<Vec<FieldMetadata>> {
    let configuration_field_names = configuration_fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<HashSet<_>>();

    command
        .get_arguments()
        .filter(|argument| is_documented_root_argument(argument))
        .filter(|argument| !configuration_field_names.contains(argument.get_id().as_str()))
        .map(|argument| documented_clap_parser_field(command, argument.get_id().as_str()))
        .collect()
}

/// Build release-help metadata for one parser-only top-level selector.
///
/// # Errors
///
/// Returns an error when the parser no longer exposes the selector, omits its
/// Fluent key, or changes its path value type.
pub(super) fn documented_clap_parser_field(
    command: &Command,
    argument_id: &str,
) -> Result<FieldMetadata> {
    let argument = command
        .get_arguments()
        .find(|argument| argument.get_id() == argument_id)
        .with_context(|| {
            format!("Cli::command() should expose its parser-only {argument_id} argument")
        })?;
    let help_id = top_level_flag_help_key(argument_id).with_context(|| {
        format!("parser-only {argument_id} argument must declare a top-level Fluent help key")
    })?;

    Ok(FieldMetadata {
        name: argument.get_id().as_str().to_owned(),
        help_id: help_id.to_owned(),
        long_help_id: None,
        value: Some(parser_only_value_type(argument)?),
        default: None,
        required: argument.is_required_set(),
        deprecated: None,
        cli: Some(cli_metadata_from_argument(argument)),
        env: None,
        file: None,
        examples: Vec::new(),
        links: Vec::new(),
        notes: Vec::new(),
    })
}

/// Return the metadata value type for a parser-only selector.
///
/// # Errors
///
/// Returns an error when a new parser-only selector requires a value type that
/// release help does not yet model.
fn parser_only_value_type(argument: &Arg) -> Result<ValueType> {
    match argument.get_id().as_str() {
        "config" | "directory" => Ok(ValueType::Path),
        argument_id => {
            bail!("parser-only {argument_id} argument requires an explicit release-help value type")
        }
    }
}
