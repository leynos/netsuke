//! Contract tests pinning release-help flag documentation to the runtime parser.
//!
//! `ortho_config`'s derive invents long and short flags from Rust field names
//! and omits fields marked `skip_cli`. Netsuke never ran that generated parser,
//! so the manual page could advertise `--default-targets` and `-d` (both
//! rejected by the binary) while never mentioning `--color`, `--emoji`,
//! `--progress`, `--accessibility` or `--no-input` (all accepted). Issue #960
//! reconciles every configuration field's CLI metadata from
//! `configured_command(None)`; these tests hold that reconciliation in both
//! directions.

use super::ReleaseHelpCli;
use super::parser_fields::is_documented_root_argument;
use crate::cli::configured_command;
use anyhow::{Context, Result, ensure};
use clap::{Arg, ArgAction, Command};
use ortho_config::docs::CliMetadata;

/// Build the runtime parser used by both parity directions.
fn runtime_command() -> Command {
    configured_command(None)
}

/// Return documented CLI metadata for `field_name`.
///
/// # Errors
///
/// Returns an error when release help omits the field or publishes no CLI
/// metadata for it.
fn documented_cli(field_name: &str) -> Result<CliMetadata> {
    ReleaseHelpCli::try_get_doc_metadata()?
        .fields
        .into_iter()
        .find(|field| field.name == field_name)
        .with_context(|| format!("release help should document {field_name}"))?
        .cli
        .with_context(|| format!("release help should publish CLI metadata for {field_name}"))
}

/// Verify that documented CLI value shapes match the parser users actually run.
///
/// `ortho_config` 0.10.0 documents every derived boolean as `--flag[=BOOL]`,
/// because its generated parser accepts an optional value. Netsuke declares its
/// flags by hand and rejects that spelling, so release help normalizes boolean
/// fields back to presence-only switches. Comparing every CLI member against
/// `configured_command(None)` makes the parity a standing contract: a future
/// metadata drift fails here instead of shipping a manual page the binary
/// contradicts.
#[test]
fn release_help_cli_value_shapes_match_the_parser() -> Result<()> {
    let metadata = ReleaseHelpCli::try_get_doc_metadata()?;
    let command = runtime_command();
    let mut compared = Vec::new();

    for field in &metadata.fields {
        let Some(documentation) = field.cli.as_ref() else {
            continue;
        };
        let argument = matching_argument(&command, &field.name)?;
        ensure_flag_identity(documentation, argument, &field.name)?;
        ensure_value_surface(documentation, argument, &field.name)?;
        compared.push(field.name.as_str());
    }

    // The four boolean flags that 0.10.0 first documented as `--flag[=BOOL]`
    // are the reason this contract exists, so their absence proves nothing.
    // The five `skip_cli` policy fields and `default-target` are issue #960's
    // regressions: before the reconciliation they were either undocumented or
    // documented under invented spellings.
    for expected in [
        "verbose",
        "fetch_default_deny",
        "trust_project_fetch_policy",
        "json",
        "default_targets",
        "color",
        "emoji",
        "progress",
        "accessibility",
        "no_input",
    ] {
        ensure!(
            compared.contains(&expected),
            "release help should compare {expected} against the parser: {compared:?}"
        );
    }
    Ok(())
}

/// Return the parser argument that documents `field_name`.
///
/// # Errors
///
/// Returns an error when release help publishes CLI metadata for a flag the
/// parser does not accept.
fn matching_argument<'command>(
    command: &'command Command,
    field_name: &str,
) -> Result<&'command Arg> {
    command
        .get_arguments()
        .find(|argument| argument.get_id().as_str() == field_name)
        .with_context(|| {
            format!(
                "release help documents {field_name}, but the parser accepts no \
                 such argument; the manual page would advertise a flag this \
                 binary rejects"
            )
        })
}

/// Verify the flag identity members of `documentation` against `argument`.
fn ensure_flag_identity(documentation: &CliMetadata, argument: &Arg, name: &str) -> Result<()> {
    ensure!(
        documentation.long.as_deref() == argument.get_long(),
        "release help should document the long flag {name} parses: \
         documented {documented:?}, parser {parsed:?}",
        documented = documentation.long,
        parsed = argument.get_long(),
    );
    ensure!(
        documentation.short == argument.get_short(),
        "release help should document the short flag {name} parses: \
         documented {documented:?}, parser {parsed:?}",
        documented = documentation.short,
        parsed = argument.get_short(),
    );
    ensure!(
        documentation.multiple == matches!(argument.get_action(), &ArgAction::Append),
        "release help should document whether {name} repeats: documented \
         {documented}, parser {parsed}",
        documented = documentation.multiple,
        parsed = matches!(argument.get_action(), &ArgAction::Append),
    );
    ensure!(
        documentation.hide_in_help == argument.is_hide_set(),
        "release help should agree with the parser on hiding {name}"
    );
    Ok(())
}

/// Verify the value-surface members of `documentation` against `argument`.
fn ensure_value_surface(documentation: &CliMetadata, argument: &Arg, name: &str) -> Result<()> {
    let takes_value = argument.get_action().takes_values();
    ensure!(
        documentation.takes_value == takes_value,
        "release help should document the value shape {name} parses: \
         documented {documented}, parser {parsed}",
        documented = documentation.takes_value,
        parsed = takes_value,
    );
    let expects_optional_value = takes_value
        && argument
            .get_num_args()
            .is_some_and(|range| range.min_values() == 0);
    ensure!(
        documentation.value_optional == expects_optional_value,
        "release help should agree with the parser on the arity of {name}: \
         documented {documented}, parser {parsed}",
        documented = documentation.value_optional,
        parsed = expects_optional_value,
    );
    ensure!(
        documentation.value_name == parser_value_name(argument),
        "release help should document the value name {name} parses: \
         documented {documented:?}, parser {parsed:?}",
        documented = documentation.value_name,
        parsed = parser_value_name(argument),
    );

    if takes_value {
        ensure!(
            documentation.possible_values == parser_possible_values(argument),
            "release help should document the accepted values for {name}"
        );
    } else {
        // A presence-only switch parses no value at all. Clap still records
        // `possible_values` for `SetTrue` actions, so copying them would
        // describe a value surface no invocation can reach.
        ensure!(
            documentation.value_name.is_none() && documentation.possible_values.is_empty(),
            "release help should describe {name} as a switch with no value"
        );
    }
    Ok(())
}

/// Return the value placeholder the parser advertises for `argument`.
fn parser_value_name(argument: &Arg) -> Option<String> {
    if argument.get_action().takes_values() {
        argument
            .get_value_names()
            .and_then(|names| names.first())
            .map(ToString::to_string)
    } else {
        None
    }
}

/// Return the accepted values the parser advertises for `argument`.
fn parser_possible_values(argument: &Arg) -> Vec<String> {
    argument
        .get_possible_values()
        .iter()
        .map(|value| value.get_name().to_owned())
        .collect()
}

/// Verify that release help documents every documentable parser argument once.
///
/// The forward parity test only inspects fields release help publishes, so a
/// field metadata drops entirely would never be examined. This reverse sweep
/// closes that hole: every root argument the binary accepts must appear as
/// exactly one documented CLI flag.
///
/// Subcommand trees stay out of scope: `documented_clap_subcommands` describes
/// subcommands shallowly, and their arguments are not part of the top-level
/// option list this contract covers.
#[test]
fn release_help_documents_every_parser_argument() -> Result<()> {
    let metadata = ReleaseHelpCli::try_get_doc_metadata()?;
    let command = runtime_command();

    let documented = metadata
        .fields
        .iter()
        .filter(|field| field.cli.is_some())
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();

    for argument in command
        .get_arguments()
        .filter(|argument| is_documented_root_argument(argument))
    {
        let argument_id = argument.get_id();
        let occurrences = documented
            .iter()
            .filter(|name| *name == argument_id)
            .count();
        ensure!(
            occurrences == 1,
            "release help should document the parser argument {argument_id} \
             exactly once, but found {occurrences}; documented flags: \
             {documented:?}"
        );
    }
    Ok(())
}

mod flag_cases;
