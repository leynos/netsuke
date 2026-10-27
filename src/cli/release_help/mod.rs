//! Release-help metadata derived from Netsuke's configuration and CLI models.
//!
//! `cargo-orthohelp` consumes [`ReleaseHelpCli`] rather than [`CliConfig`]
//! directly. The adapter retains the configuration-field metadata generated
//! for `CliConfig` and adds parser-only arguments and Clap subcommands that
//! users can invoke.

use anyhow::{Context, Result};
use clap::CommandFactory;
use ortho_config::docs::{DocMetadata, FieldMetadata, OrthoConfigDocs, ValueType};

use super::{Cli, CliConfig};
use crate::{cli::l10n::top_level_flag_help_key, localization::keys};

mod parser_fields;

use parser_fields::documented_clap_parser_fields;

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
    /// Return the composed metadata consumed by release-help generators.
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
///
/// # Errors
///
/// Returns an error when a published configuration field has no declared
/// top-level Fluent help key.
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
            presence_only_cli_metadata(&mut field);
            Ok(field)
        })
        .collect()
}

/// Describe a boolean configuration flag as the presence-only switch Netsuke
/// exposes.
///
/// `ortho_config` 0.10.0 documents every boolean field as `--flag[=BOOL]`,
/// because its *generated* clap parser accepts an optional value. Netsuke does
/// not use that parser: `src/cli/command.rs` declares the flags by hand, and
/// they reject `--verbose=false` with clap's `unexpected value` error. Release
/// help must describe the parser users actually run, so the generated
/// optional-value surface is reduced to the plain switch that exists. Without
/// this, the man page would document a spelling the binary refuses.
fn presence_only_cli_metadata(field: &mut FieldMetadata) {
    if !matches!(field.value.as_ref(), Some(ValueType::Bool)) {
        return;
    }
    let Some(cli) = field.cli.as_mut() else {
        return;
    };
    cli.takes_value = false;
    cli.value_name = None;
    cli.value_optional = false;
    cli.possible_values.clear();
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
        // Netsuke declares no behavioural metadata and no profile selector.
        behaviour: None,
        profiles: None,
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
mod metadata_tests;

#[cfg(test)]
mod tests {
    //! Tests for release-help metadata assembled from the Clap command tree.

    use super::metadata_tests::{inert_field_metadata, recognised_configuration_field_names};
    use super::*;
    use anyhow::{Context, Result, ensure};
    use proptest::prelude::*;

    /// Resolve a release-help metadata description through the English localizer.
    ///
    /// # Errors
    ///
    /// Returns an error when the requested metadata or its Fluent message is absent.
    fn localized_release_help_description(
        find_help_id: impl FnOnce(&DocMetadata) -> Option<String>,
        missing_message: &'static str,
    ) -> Result<String> {
        let metadata = ReleaseHelpCli::get_doc_metadata();
        let help_id = find_help_id(&metadata).context(missing_message)?;
        crate::cli_localization::build_localizer(Some("en-US"))
            .lookup(&help_id, None)
            .context("release help metadata should resolve its description")
    }

    /// Verify that the help subcommand documents the available help targets.
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

    /// Verify that Cargo selects the release-help Clap metadata adapter.
    #[test]
    fn cargo_metadata_selects_the_clap_documentation_adapter() {
        assert!(
            include_str!("../../../Cargo.toml")
                .contains("root_type = \"netsuke::cli::ReleaseHelpCli\""),
            "cargo-orthohelp should load the metadata that includes Clap subcommands"
        );
    }

    /// Verify that the help-targets description resolves through localization.
    #[test]
    fn release_help_metadata_localizes_the_help_targets_description() -> Result<()> {
        let description = localized_release_help_description(
            |metadata| {
                metadata
                    .subcommands
                    .iter()
                    .find(|command| command.app_name == "help")
                    .map(|command| command.about_id.clone())
            },
            "release help metadata should include the help command",
        )?;

        ensure!(
            description.contains("help targets"),
            "release help description should document the targets topic: {description}"
        );
        Ok(())
    }

    /// Verify that the configuration selector description resolves through localization.
    #[test]
    fn release_help_metadata_localizes_the_config_description() -> Result<()> {
        let description = localized_release_help_description(
            |metadata| {
                metadata
                    .fields
                    .iter()
                    .find(|field| field.name == "config")
                    .map(|field| field.help_id.clone())
            },
            "release help metadata should include the config selector",
        )?;

        ensure!(
            description == "Path to a configuration file, bypassing automatic discovery.",
            "release help config description should be localized: {description}"
        );
        Ok(())
    }

    /// Verify that every published field has a resolvable localized description.
    ///
    /// # Errors
    ///
    /// Returns an error when release-help metadata omits a field property or its
    /// Fluent description cannot be resolved.
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

    proptest! {
        /// Verify that configuration metadata is projected or rejected consistently.
        #[test]
        fn localized_config_help_preserves_the_configuration_metadata_contract(
            field_names in prop::collection::vec(
                prop_oneof![
                    3 => prop::sample::select(recognised_configuration_field_names()),
                    1 => Just("cmds".to_owned()),
                    1 => "unknown[a-z]{0,12}".prop_map(|suffix| format!("unknown{suffix}")),
                ],
                0..64,
            ),
        ) {
            let contains_unrecognised_field = field_names.iter().any(|name| {
                name != "cmds" && top_level_flag_help_key(name).is_none()
            });
            let expected_recognised_names = field_names
                .iter()
                .filter(|name| top_level_flag_help_key(name).is_some())
                .cloned()
                .collect::<Vec<_>>();
            let projection = localized_config_help(
                field_names
                    .iter()
                    .cloned()
                    .map(inert_field_metadata)
                    .collect(),
            );

            if contains_unrecognised_field {
                prop_assert!(projection.is_err());
            } else {
                let projected_fields = projection
                    .map_err(|error| TestCaseError::fail(error.to_string()))?;

                prop_assert_eq!(
                    projected_fields.iter().map(|field| field.name.clone()).collect::<Vec<_>>(),
                    expected_recognised_names,
                );
                let has_valid_localized_fields = projected_fields.iter().all(|field| {
                    field.name != "cmds"
                        && field.long_help_id.is_none()
                        && top_level_flag_help_key(&field.name) == Some(field.help_id.as_str())
                });
                prop_assert!(has_valid_localized_fields);
            }
        }
    }
}
