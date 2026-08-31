//! Release-help metadata derived from Netsuke's configuration and CLI models.
//!
//! `cargo-orthohelp` consumes [`ReleaseHelpCli`] rather than [`CliConfig`]
//! directly. The adapter retains the configuration-field metadata generated
//! for `CliConfig` and adds parser-only arguments and Clap subcommands that
//! users can invoke.

use anyhow::{Context, Result, bail};
use clap::{Arg, ArgAction, Command, CommandFactory};
use ortho_config::docs::{CliMetadata, DocMetadata, FieldMetadata, OrthoConfigDocs, ValueType};
use std::collections::HashSet;

use super::{Cli, CliConfig};
use crate::{cli_l10n::top_level_flag_help_key, localization::keys};

/// Documentation root used by release help generators.
///
/// ```
/// use netsuke::cli::ReleaseHelpCli;
/// use ortho_config::docs::OrthoConfigDocs;
///
/// let metadata = ReleaseHelpCli::get_doc_metadata();
/// assert!(metadata.subcommands.iter().any(|command| command.app_name == "help"));
/// ```
pub struct ReleaseHelpCli;

impl ReleaseHelpCli {
    /// Compose complete release-help metadata from configuration and parser sources.
    fn try_get_doc_metadata() -> Result<DocMetadata> {
        let mut metadata = CliConfig::get_doc_metadata();
        keys::CLI_ABOUT.clone_into(&mut metadata.about_id);
        metadata.fields = localized_config_help(metadata.fields)?;
        let parser_fields = documented_clap_parser_fields(&Cli::command(), &metadata.fields)?;
        metadata.fields.extend(parser_fields);
        metadata.subcommands = documented_clap_subcommands(&metadata);
        Ok(metadata)
    }
}

impl OrthoConfigDocs for ReleaseHelpCli {
    fn get_doc_metadata() -> DocMetadata {
        match Self::try_get_doc_metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                panic!("release-help metadata contract is invalid: {error}");
            }
        }
    }
}

/// Project existing CLI Fluent keys onto release-help configuration fields.
///
/// `CliConfig` remains the source of the fields and their configuration
/// sources. The structural `cmds` container is not a public configuration
/// setting, so release help omits it rather than inventing a separate model.
fn localized_config_help(fields: Vec<FieldMetadata>) -> Result<Vec<FieldMetadata>> {
    fields
        .into_iter()
        .filter(|field| field.name != "cmds")
        .map(|mut field| {
            let help_key = top_level_flag_help_key(&field.name).with_context(|| {
                format!(
                    "release-help configuration field {} must declare a top-level Fluent help key",
                    field.name
                )
            })?;
            help_key.clone_into(&mut field.help_id);
            field.long_help_id = None;
            Ok(field)
        })
        .collect()
}

/// Build release-help metadata for every parser-only top-level selector.
///
/// The selectors have no configuration, environment, or file source. Discovery
/// retains their policy in `discovery.rs` under ADR 004.
///
/// # Errors
///
/// Returns an error when a parser-only selector lacks a Fluent key or path
/// metadata, which would leave generated release help incomplete.
fn documented_clap_parser_fields(
    command: &Command,
    configuration_fields: &[FieldMetadata],
) -> Result<Vec<FieldMetadata>> {
    let configuration_field_names = configuration_fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<HashSet<_>>();

    command
        .get_arguments()
        .filter(|argument| argument.get_id() != "help")
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
fn documented_clap_parser_field(command: &Command, argument_id: &str) -> Result<FieldMetadata> {
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
        cli: Some(CliMetadata {
            long: argument.get_long().map(str::to_owned),
            short: argument.get_short(),
            value_name: argument
                .get_value_names()
                .and_then(|names| names.first())
                .map(ToString::to_string),
            multiple: matches!(argument.get_action(), &ArgAction::Append),
            takes_value: argument.get_action().takes_values(),
            possible_values: argument
                .get_possible_values()
                .iter()
                .map(|value| value.get_name().to_owned())
                .collect(),
            hide_in_help: argument.is_hide_set(),
        }),
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

/// Return documentation metadata for every Clap subcommand with an about key.
fn documented_clap_subcommands(root: &DocMetadata) -> Vec<DocMetadata> {
    Cli::command()
        .get_subcommands()
        .filter_map(|command| {
            release_help_about_key(command.get_name())
                .map(|about_id| documented_subcommand(root, command.get_name(), about_id))
        })
        .collect()
}

/// Build documentation metadata for `name`, reusing the root's shared fields.
fn documented_subcommand(root: &DocMetadata, name: &str, about_id: &str) -> DocMetadata {
    DocMetadata {
        ir_version: root.ir_version.clone(),
        app_name: name.to_owned(),
        bin_name: Some(name.to_owned()),
        about_id: about_id.to_owned(),
        synopsis_id: None,
        sections: root.sections.clone(),
        fields: Vec::new(),
        subcommands: Vec::new(),
        windows: None,
    }
}

/// Return the localization key for `name`'s about text, or `None` when the
/// subcommand has no release documentation.
fn release_help_about_key(name: &str) -> Option<&'static str> {
    match name {
        "build" => Some(keys::CLI_SUBCOMMAND_BUILD_ABOUT),
        "clean" => Some(keys::CLI_SUBCOMMAND_CLEAN_ABOUT),
        "graph" => Some(keys::CLI_SUBCOMMAND_GRAPH_ABOUT),
        "generate" => Some(keys::CLI_SUBCOMMAND_GENERATE_ABOUT),
        // The long description is the release artefact's only representation
        // of nested help topics, so retain the `help targets` invocation.
        "help" => Some(keys::CLI_SUBCOMMAND_HELP_LONG_ABOUT),
        _ => None,
    }
}

#[cfg(test)]
#[path = "release_help_metadata_tests.rs"]
mod metadata_tests;

#[cfg(test)]
mod tests {
    //! Tests for release-help metadata assembled from the Clap command tree.

    use super::*;
    use anyhow::{Context, Result, ensure};

    #[test]
    fn metadata_documents_help_targets_through_the_help_subcommand() {
        let metadata = ReleaseHelpCli::get_doc_metadata();
        let help = metadata
            .subcommands
            .iter()
            .find(|command| command.app_name == "help")
            .expect("Clap help command should be present in release metadata");

        assert_eq!(help.about_id, keys::CLI_SUBCOMMAND_HELP_LONG_ABOUT);
        assert_eq!(
            metadata
                .subcommands
                .iter()
                .map(|command| command.app_name.as_str())
                .collect::<Vec<_>>(),
            ["build", "clean", "graph", "generate", "help"]
        );
    }

    #[test]
    fn cargo_metadata_selects_the_clap_documentation_adapter() {
        assert!(
            include_str!("../../Cargo.toml")
                .contains("root_type = \"netsuke::cli::ReleaseHelpCli\""),
            "cargo-orthohelp should load the metadata that includes Clap subcommands"
        );
    }

    #[test]
    fn release_help_metadata_localizes_the_help_targets_description() -> Result<()> {
        let metadata = ReleaseHelpCli::get_doc_metadata();
        let help = metadata
            .subcommands
            .iter()
            .find(|command| command.app_name == "help")
            .context("release help metadata should include the help command")?;
        let localizer = crate::cli_localization::build_localizer(Some("en-US"));
        let description = localizer
            .lookup(&help.about_id, None)
            .context("release help metadata should resolve its help description")?;

        ensure!(
            description.contains("help targets"),
            "release help description should document the targets topic: {description}"
        );
        Ok(())
    }

    /// Verify that the configuration selector description resolves through localization.
    #[test]
    fn release_help_metadata_localizes_the_config_description() -> Result<()> {
        let metadata = ReleaseHelpCli::get_doc_metadata();
        let config = metadata
            .fields
            .iter()
            .find(|field| field.name == "config")
            .context("release help metadata should include the config selector")?;
        let localizer = crate::cli_localization::build_localizer(Some("en-US"));
        let description = localizer
            .lookup(&config.help_id, None)
            .context("release help metadata should resolve its config description")?;

        ensure!(
            description == "Path to a configuration file, bypassing automatic discovery.",
            "release help config description should be localized: {description}"
        );
        Ok(())
    }

    #[test]
    fn release_help_metadata_localizes_every_published_field() -> Result<()> {
        let metadata = ReleaseHelpCli::get_doc_metadata();
        let localizer = crate::cli_localization::build_localizer(Some("en-US"));

        for field in metadata.fields {
            ensure!(
                field.name != "cmds",
                "release help must not expose the structural cmds container"
            );
            ensure!(
                field.long_help_id.is_none(),
                "release help should use one resolved help key for {}",
                field.name
            );
            let description = localizer
                .lookup(&field.help_id, None)
                .with_context(|| format!("release help should localize {}", field.name))?;
            ensure!(
                !description.starts_with("[missing:"),
                "release help should not emit a missing message for {}: {description}",
                field.name
            );
        }
        Ok(())
    }
}
