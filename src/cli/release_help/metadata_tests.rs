//! Unit tests for the complete release-help metadata contract.
//!
//! The tests cover the configuration-help projection, the parser-only field
//! and value-shape derivation, and the identifiers carried by the composed
//! root and immediate subcommand documentation metadata.

use super::*;
use anyhow::{Context, Result, ensure};
use clap::{Arg, Command};
use ortho_config::docs::CliMetadata;
use rstest::rstest;

use super::parser_fields::documented_clap_parser_field;

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

/// Find the parser argument a documented field claims to describe.
///
/// # Errors
///
/// Returns an error when the parser accepts no argument with that identifier,
/// which would mean the manual page advertises a flag the binary rejects.
fn find_parser_argument<'a>(command: &'a Command, field_name: &str) -> Result<&'a Arg> {
    command
        .get_arguments()
        .find(|argument| argument.get_id().as_str() == field_name)
        .with_context(|| {
            format!(
                "release help documents {field_name}, but the parser accepts no such \
                 argument; the manual page would advertise a flag this binary rejects"
            )
        })
}

/// Verify one documented field against the parser argument it describes.
///
/// # Errors
///
/// Returns an error when the value shape, placeholder, or accepted values
/// diverge from what the parser accepts.
fn compare_argument_value_shape(
    field: &FieldMetadata,
    documentation: &CliMetadata,
    argument: &Arg,
) -> Result<()> {
    let takes_value = argument.get_action().takes_values();
    ensure!(
        documentation.takes_value == takes_value,
        "release help should document the value shape {name} parses: \
         documented {documented}, parser {parsed}",
        name = field.name,
        documented = documentation.takes_value,
        parsed = takes_value,
    );
    ensure!(
        !documentation.value_optional,
        "release help must not offer the optional-value spelling for {}",
        field.name
    );
    if takes_value {
        compare_value_taking_argument(field, documentation, argument)?;
    } else {
        // A presence-only switch parses no value at all. Clap still records
        // `possible_values` for `SetTrue` actions, so copying them would
        // describe a value surface no invocation can reach.
        ensure!(
            documentation.value_name.is_none() && documentation.possible_values.is_empty(),
            "release help should describe {} as a switch with no value",
            field.name
        );
    }
    Ok(())
}

/// Verify the placeholder and accepted values of a value-taking argument.
///
/// # Errors
///
/// Returns an error when the parser declares no value name, when the documented
/// placeholder disagrees with it, or when the accepted values differ.
fn compare_value_taking_argument(
    field: &FieldMetadata,
    documentation: &CliMetadata,
    argument: &Arg,
) -> Result<()> {
    let parser_value_name = argument
        .get_value_names()
        .and_then(|names| names.first())
        .map(ToString::to_string);
    ensure!(
        parser_value_name.is_some(),
        "parser argument {} should declare a value name to document",
        field.name
    );
    ensure!(
        documentation.value_name == parser_value_name,
        "release help should document the value placeholder {name} \
         parses: documented {documented:?}, parser {parsed:?}",
        name = field.name,
        documented = documentation.value_name,
        parsed = parser_value_name,
    );
    ensure!(
        documentation.possible_values
            == argument
                .get_possible_values()
                .iter()
                .map(|value| value.get_name().to_owned())
                .collect::<Vec<_>>(),
        "release help should document the accepted values for {}",
        field.name
    );
    Ok(())
}

/// Verify that documented CLI value shapes match the parser users actually run.
///
/// `ortho_config` 0.10.0 documents every derived boolean as `--flag[=BOOL]`,
/// because its generated parser accepts an optional value. Netsuke declares its
/// flags by hand and rejects that spelling, so release help normalizes boolean
/// fields back to presence-only switches. Comparing against `Cli::command()`
/// makes the parity a standing contract: a future metadata drift fails here
/// instead of shipping a manual page the binary contradicts.
///
/// Value names are compared too. `CliConfig` declares the placeholder for every
/// configuration-derived option through `cli(value_name = "...")`; without that
/// declaration the generator falls back to a machine type such as `UTF8PATHBUF`
/// or `USIZE`, which no reader of the manual page should see. A name missing
/// here, or one that disagrees with the parser, fails the test.
#[test]
fn release_help_cli_value_shapes_match_the_parser() -> Result<()> {
    let metadata = ReleaseHelpCli::try_get_doc_metadata()?;
    let command = Cli::command();
    let mut compared = Vec::new();

    for field in &metadata.fields {
        let Some(documentation) = field.cli.as_ref() else {
            continue;
        };
        let argument = find_parser_argument(&command, &field.name)?;
        compare_argument_value_shape(field, documentation, argument)?;
        compared.push(field.name.as_str());
    }

    // The four boolean flags that 0.10.0 first documented as `--flag[=BOOL]`
    // are the reason this contract exists, so their absence proves nothing.
    for expected in [
        "verbose",
        "fetch_default_deny",
        "trust_project_fetch_policy",
        "json",
    ] {
        ensure!(
            compared.contains(&expected),
            "release help should compare {expected} against the parser: {compared:?}"
        );
    }

    // Value-taking fields are where the generator's `UTF8PATHBUF` and `USIZE`
    // fallbacks appeared, so their absence from the comparison would hide the
    // very regression this contract now guards against.
    for expected in ["file", "jobs", "locale", "manifest_fuel", "default_targets"] {
        ensure!(
            compared.contains(&expected),
            "release help should compare the value placeholder for {expected}: {compared:?}"
        );
    }
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

/// Verify that the release-help identifiers this test reads are Netsuke-declared.
///
/// The v0.10.0 migration guide describes re-keying catalogues from
/// `{app}.fields.{field}.help` to `{mounted-path}-args-{field}-help`. Netsuke
/// needs none of that because `localized_config_help` replaces every generated
/// `help_id` with its own `define_keys!` identifier before release help sees it.
/// This test pins the consequence for the identifier kinds it collects:
/// configuration-field `help_id`, the root `about_id` and `synopsis_id`, and
/// each immediate subcommand's `about_id` and `synopsis_id`. Every collected
/// identifier must be a key `build_l10n_audit` can attribute to
/// `src/localization`, and none may carry the upstream shapes. Were the
/// projection to stop applying, the audit would not catch it — the generated
/// keys are absent from the catalogues, so a stray one becomes a runtime miss
/// rather than a build failure.
///
/// Section heading ids and `long_help_id` are not collected here, so this is
/// not a complete census of every identifier the IR can carry.
#[test]
fn release_help_identifiers_are_netsuke_declared_not_generated() -> Result<()> {
    let metadata = ReleaseHelpCli::try_get_doc_metadata()?;
    let mut identifiers = metadata
        .fields
        .iter()
        .map(|field| field.help_id.as_str())
        .chain(std::iter::once(metadata.about_id.as_str()))
        .chain(metadata.synopsis_id.as_deref())
        .collect::<Vec<_>>();
    identifiers.extend(metadata.subcommands.iter().flat_map(|command| {
        std::iter::once(command.about_id.as_str()).chain(command.synopsis_id.as_deref())
    }));

    let generated_shapes = [".fields.", "-args-"];
    for identifier in &identifiers {
        for shape in generated_shapes {
            ensure!(
                !identifier.contains(shape),
                "release help should not emit the generated identifier {identifier}"
            );
        }
        ensure!(
            identifier.starts_with("cli."),
            "release help should name a netsuke-declared key, got {identifier}"
        );
    }

    // The localizer resolves against `locales/<tag>/messages.ftl`, so a key that
    // is missing there would surface as a runtime `[missing: ...]` string.
    let localizer = crate::cli_localization::build_localizer(Some("en-US"));
    for identifier in identifiers {
        let message = localizer
            .lookup(identifier, None)
            .with_context(|| format!("release help should resolve {identifier}"))?;
        ensure!(
            !message.starts_with("[missing:"),
            "release help identifier {identifier} should exist in the catalogue: {message}"
        );
    }
    Ok(())
}
