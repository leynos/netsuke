//! Release-help metadata derived from Netsuke's configuration and CLI models.
//!
//! `cargo-orthohelp` consumes [`ReleaseHelpCli`] rather than [`CliConfig`]
//! directly. The adapter retains the configuration-field metadata generated
//! for `CliConfig` and adds parser-only arguments and Clap subcommands that
//! users can invoke.

use anyhow::{Context, Result};
use clap::{ArgAction, Command, CommandFactory};
use ortho_config::docs::{CliMetadata, DocMetadata, FieldMetadata, OrthoConfigDocs, ValueType};

use super::{Cli, CliConfig};
use crate::localization::keys;

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

impl OrthoConfigDocs for ReleaseHelpCli {
    fn get_doc_metadata() -> DocMetadata {
        let mut metadata = CliConfig::get_doc_metadata();
        keys::CLI_ABOUT.clone_into(&mut metadata.about_id);
        localized_config_help(&mut metadata.fields);
        match documented_clap_config_field(&Cli::command()) {
            Ok(config) => metadata.fields.push(config),
            Err(error) => {
                tracing::error!(
                    ?error,
                    "release help omits parser-only config metadata because the parser contract changed"
                );
            }
        }
        metadata.subcommands = documented_clap_subcommands(&metadata);
        metadata
    }
}

/// Project existing CLI Fluent keys onto release-help configuration fields.
///
/// `CliConfig` remains the source of the fields and their configuration
/// sources. The structural `cmds` container is not a public configuration
/// setting, so release help omits it rather than inventing a separate model.
fn localized_config_help(fields: &mut Vec<FieldMetadata>) {
    fields.retain_mut(|field| {
        let Some(help_key) = localized_config_help_key(&field.name) else {
            tracing::error!(
                field = %field.name,
                "release help omits a configuration field without a declared Fluent help key"
            );
            return false;
        };
        help_key.clone_into(&mut field.help_id);
        field.long_help_id = None;
        true
    });
}

/// Return the existing Fluent key that documents a published configuration field.
fn localized_config_help_key(name: &str) -> Option<&'static str> {
    Some(match name {
        "file" => keys::CLI_FLAG_FILE_HELP,
        "jobs" => keys::CLI_FLAG_JOBS_HELP,
        "verbose" => keys::CLI_FLAG_VERBOSE_HELP,
        "locale" => keys::CLI_FLAG_LOCALE_HELP,
        "fetch_allow_scheme" => keys::CLI_FLAG_FETCH_ALLOW_SCHEME_HELP,
        "fetch_allow_host" => keys::CLI_FLAG_FETCH_ALLOW_HOST_HELP,
        "fetch_block_host" => keys::CLI_FLAG_FETCH_BLOCK_HOST_HELP,
        "fetch_default_deny" => keys::CLI_FLAG_FETCH_DEFAULT_DENY_HELP,
        "json" => keys::CLI_FLAG_JSON_HELP,
        "no_input" => keys::CLI_FLAG_NO_INPUT_HELP,
        "color" => keys::CLI_FLAG_COLOR_HELP,
        "emoji" => keys::CLI_FLAG_EMOJI_HELP,
        "progress" => keys::CLI_FLAG_PROGRESS_HELP,
        "accessibility" => keys::CLI_FLAG_ACCESSIBILITY_HELP,
        "default_targets" => keys::CLI_FLAG_DEFAULT_TARGETS_HELP,
        _ => return None,
    })
}

/// Build release-help metadata for Netsuke's parser-only `--config` selector.
///
/// The selector has no configuration, environment, or file source: discovery
/// retains that policy in `discovery.rs` under ADR 004.
///
/// # Errors
///
/// Returns an error when the parser no longer exposes the required `config`
/// argument, which would leave generated release help incomplete.
fn documented_clap_config_field(command: &Command) -> Result<FieldMetadata> {
    let config = command
        .get_arguments()
        .find(|argument| argument.get_id() == "config")
        .context("Cli::command() should expose its parser-only config argument")?;

    Ok(FieldMetadata {
        name: config.get_id().as_str().to_owned(),
        help_id: keys::CLI_FLAG_CONFIG_HELP.to_owned(),
        long_help_id: None,
        value: Some(ValueType::Path),
        default: None,
        required: config.is_required_set(),
        deprecated: None,
        cli: Some(CliMetadata {
            long: config.get_long().map(str::to_owned),
            short: config.get_short(),
            value_name: config
                .get_value_names()
                .and_then(|names| names.first())
                .map(ToString::to_string),
            multiple: matches!(config.get_action(), &ArgAction::Append),
            takes_value: config.get_action().takes_values(),
            possible_values: config
                .get_possible_values()
                .iter()
                .map(|value| value.get_name().to_owned())
                .collect(),
            hide_in_help: config.is_hide_set(),
        }),
        env: None,
        file: None,
        examples: Vec::new(),
        links: Vec::new(),
        notes: Vec::new(),
    })
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
mod tests {
    //! Tests for release-help metadata assembled from the Clap command tree.

    use super::*;
    use anyhow::{Context, Result, ensure};
    use rstest::rstest;

    #[rstest]
    #[case(Cli::command(), Some("config"))]
    #[case(Command::new("netsuke"), None)]
    fn config_metadata_requires_the_parser_argument(
        #[case] command: Command,
        #[case] expected_config_long: Option<&str>,
    ) -> Result<()> {
        let extraction = documented_clap_config_field(&command);

        if let Some(config_long) = expected_config_long {
            let extracted_field =
                extraction.context("config argument should produce release-help metadata")?;
            let cli = extracted_field
                .cli
                .context("config metadata should retain its CLI source")?;
            ensure!(
                extracted_field.name == "config",
                "config metadata should retain its name"
            );
            ensure!(
                extracted_field.help_id == keys::CLI_FLAG_CONFIG_HELP,
                "config metadata should use the localized config help key"
            );
            ensure!(
                cli.long.as_deref() == Some(config_long),
                "config metadata should retain its long flag"
            );
            ensure!(
                cli.value_name.as_deref() == Some("FILE"),
                "config metadata should retain its path value name"
            );
            ensure!(cli.takes_value, "config selector should accept a path");
            ensure!(
                extracted_field.env.is_none(),
                "config selector must not gain an environment source"
            );
            ensure!(
                extracted_field.file.is_none(),
                "config selector must not gain a file source"
            );
        } else {
            let error = extraction
                .err()
                .context("missing config argument should fail metadata extraction")?;
            ensure!(
                error.to_string().contains("parser-only config argument"),
                "missing config error should identify the parser contract: {error}"
            );
        }
        Ok(())
    }

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
