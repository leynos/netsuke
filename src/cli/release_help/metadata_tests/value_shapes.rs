//! Value-shape parity between release-help metadata and the parser.
//!
//! These checks read the composed metadata and `Cli::command()` together, so
//! they live apart from the parser-only and localization tests that build the
//! metadata in the first place.

use anyhow::{Context, Result, ensure};
use clap::{Arg, Command, CommandFactory};
use ortho_config::docs::{CliMetadata, FieldMetadata};

use crate::cli::{Cli, ReleaseHelpCli};

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
