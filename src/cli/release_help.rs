//! Release-help metadata derived from Netsuke's configuration and CLI models.
//!
//! `cargo-orthohelp` consumes [`ReleaseHelpCli`] rather than [`CliConfig`]
//! directly. The adapter retains the configuration-field metadata generated
//! for `CliConfig` and adds the Clap subcommands that users can invoke.

use clap::CommandFactory;
use ortho_config::docs::{DocMetadata, OrthoConfigDocs};

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
        metadata.subcommands = documented_clap_subcommands(&metadata);
        metadata
    }
}

fn documented_clap_subcommands(root: &DocMetadata) -> Vec<DocMetadata> {
    Cli::command()
        .get_subcommands()
        .filter_map(|command| {
            release_help_about_key(command.get_name())
                .map(|about_id| documented_subcommand(root, command.get_name(), about_id))
        })
        .collect()
}

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
}
