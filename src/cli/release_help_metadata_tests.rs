//! Unit tests for parser-only release-help metadata composition.

use super::*;
use anyhow::{Context, Result, ensure};
use rstest::rstest;

struct ParserOnlyFieldExpectation {
    long: &'static str,
    short: Option<char>,
    value_name: &'static str,
    help_id: &'static str,
}

/// Build inert metadata for testing the configuration-help projection.
pub(super) fn inert_field_metadata(name: String) -> FieldMetadata {
    FieldMetadata {
        name,
        help_id: "inert-help".to_owned(),
        long_help_id: Some("inert-long-help".to_owned()),
        value: None,
        default: None,
        required: false,
        deprecated: None,
        cli: None,
        env: None,
        file: None,
        examples: Vec::new(),
        links: Vec::new(),
        notes: Vec::new(),
    }
}

/// Return generated names for configuration fields with declared Fluent keys.
pub(super) fn recognised_configuration_field_names() -> Vec<String> {
    CliConfig::get_doc_metadata()
        .fields
        .into_iter()
        .filter(|field| field.name != "cmds")
        .map(|field| field.name)
        .collect()
}

/// Verify that parser-only metadata preserves the corresponding Clap details.
#[rstest]
#[case::directory(
    "directory",
    ParserOnlyFieldExpectation {
        long: "directory",
        short: Some('C'),
        value_name: "DIR",
        help_id: keys::CLI_FLAG_DIRECTORY_HELP,
    }
)]
#[case::config(
    "config",
    ParserOnlyFieldExpectation {
        long: "config",
        short: None,
        value_name: "FILE",
        help_id: keys::CLI_FLAG_CONFIG_HELP,
    }
)]
fn parser_only_metadata_preserves_clap_details(
    #[case] argument_id: &str,
    #[case] expected: ParserOnlyFieldExpectation,
) -> Result<()> {
    let extracted_field = documented_clap_parser_field(&Cli::command(), argument_id)
        .with_context(|| format!("{argument_id} argument should produce release-help metadata"))?;
    let cli = extracted_field
        .cli
        .context("parser-only metadata should retain its CLI source")?;

    ensure!(
        extracted_field.name == argument_id,
        "parser-only metadata should retain its name"
    );
    ensure!(
        extracted_field.help_id == expected.help_id,
        "{argument_id} metadata should use its localized help key"
    );
    ensure!(
        cli.long.as_deref() == Some(expected.long),
        "{argument_id} metadata should retain its long flag"
    );
    ensure!(
        cli.short == expected.short,
        "{argument_id} metadata should retain its short flag"
    );
    ensure!(
        cli.value_name.as_deref() == Some(expected.value_name),
        "{argument_id} metadata should retain its path value name"
    );
    ensure!(
        cli.takes_value,
        "{argument_id} selector should accept a path"
    );
    ensure!(
        extracted_field.env.is_none(),
        "{argument_id} selector must not gain an environment source"
    );
    ensure!(
        extracted_field.file.is_none(),
        "{argument_id} selector must not gain a file source"
    );
    Ok(())
}

/// Verify that missing parser arguments are rejected during extraction.
#[rstest]
#[case::directory("directory")]
#[case::config("config")]
fn parser_only_metadata_requires_the_parser_argument(#[case] argument_id: &str) -> Result<()> {
    let error = documented_clap_parser_field(&Command::new("netsuke"), argument_id)
        .err()
        .with_context(|| {
            format!("missing {argument_id} argument should fail metadata extraction")
        })?;

    ensure!(
        error
            .to_string()
            .contains(&format!("parser-only {argument_id} argument")),
        "missing parser-only argument should identify the parser contract: {error}"
    );
    Ok(())
}

/// Verify that release help includes every parser-only selector.
#[test]
fn release_help_metadata_composes_every_parser_only_selector() -> Result<()> {
    let metadata = ReleaseHelpCli::try_get_doc_metadata()?;
    let parser_only_field_names = metadata
        .fields
        .iter()
        .filter(|field| field.env.is_none() && field.file.is_none())
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();

    ensure!(
        parser_only_field_names == ["directory", "config"],
        "release help should compose all parser-only selectors: {parser_only_field_names:?}"
    );
    Ok(())
}

/// Verify that known fields resolve and the structural field is omitted.
#[rstest]
#[case::known("file", Some(keys::CLI_FLAG_FILE_HELP))]
#[case::structural("cmds", None)]
fn config_metadata_localization_handles_known_and_structural_fields(
    #[case] field_name: &str,
    #[case] expected_help_id: Option<&str>,
) -> Result<()> {
    let mut field = CliConfig::get_doc_metadata()
        .fields
        .into_iter()
        .next()
        .context("CliConfig metadata should include a field fixture")?;
    field.name = field_name.to_owned();

    let localized_fields = localized_config_help(vec![field])?;

    match expected_help_id {
        Some(help_id) => {
            ensure!(
                localized_fields.len() == 1,
                "known field should remain in release-help metadata"
            );
            let localized_field = localized_fields
                .first()
                .context("known field should remain in release-help metadata")?;
            ensure!(
                localized_field.help_id == help_id,
                "known field should use the shared Fluent help key"
            );
        }
        None => ensure!(
            localized_fields.is_empty(),
            "the structural cmds field should be omitted from release-help metadata"
        ),
    }
    Ok(())
}

/// Verify that unknown configuration fields fail localization.
#[test]
fn config_metadata_localization_rejects_unknown_fields() -> Result<()> {
    let mut field = CliConfig::get_doc_metadata()
        .fields
        .into_iter()
        .next()
        .context("CliConfig metadata should include a field fixture")?;
    field.name = "unknown".to_owned();

    let error = localized_config_help(vec![field])
        .err()
        .context("unknown configuration metadata should be rejected")?;
    ensure!(
        error
            .to_string()
            .contains("must declare a top-level Fluent help key"),
        "unknown field error should identify the missing Fluent key: {error}"
    );
    Ok(())
}
