//! Per-flag release-help shape cases.
//!
//! Each case pins one flag the derive misdescribed: an invented short alias,
//! a missing `skip_cli` field, or a value name the parser spells differently.

use super::documented_cli;
use anyhow::{Result, ensure};
use rstest::rstest;

/// Expected release-help shape for one parser flag.
struct FlagShapeExpectation {
    /// Long flag spelling, without leading dashes.
    long: &'static str,
    /// Value placeholder the parser advertises, if it takes a value.
    value_name: Option<&'static str>,
    /// Whether the flag accepts a value.
    takes_value: bool,
    /// Whether the flag may be repeated.
    multiple: bool,
    /// Accepted values in parser order.
    possible_values: &'static [&'static str],
}

/// Verify that policy and selector flags are documented exactly as parsed.
///
/// Every case has no short alias: the derive used to invent `-d`, `-l`, `-F`
/// and `-J`, each of which the binary rejects. The five policy fields carry
/// `skip_cli`, so before issue #960 they had no documentation at all.
#[rstest]
#[case::default_targets(
    "default_targets",
    FlagShapeExpectation {
        long: "default-target",
        value_name: Some("TARGET"),
        takes_value: true,
        multiple: true,
        possible_values: &[],
    }
)]
#[case::color(
    "color",
    FlagShapeExpectation {
        long: "color",
        value_name: Some("POLICY"),
        takes_value: true,
        multiple: false,
        possible_values: &["auto", "always", "never"],
    }
)]
#[case::emoji(
    "emoji",
    FlagShapeExpectation {
        long: "emoji",
        value_name: Some("POLICY"),
        takes_value: true,
        multiple: false,
        possible_values: &["auto", "always", "never"],
    }
)]
#[case::progress(
    "progress",
    FlagShapeExpectation {
        long: "progress",
        value_name: Some("POLICY"),
        takes_value: true,
        multiple: false,
        possible_values: &["auto", "always", "never"],
    }
)]
#[case::accessibility(
    "accessibility",
    FlagShapeExpectation {
        long: "accessibility",
        value_name: Some("POLICY"),
        takes_value: true,
        multiple: false,
        possible_values: &["auto", "on", "off"],
    }
)]
#[case::no_input(
    "no_input",
    FlagShapeExpectation {
        long: "no-input",
        value_name: None,
        takes_value: false,
        multiple: false,
        possible_values: &[],
    }
)]
#[case::locale(
    "locale",
    FlagShapeExpectation {
        long: "locale",
        value_name: Some("LOCALE"),
        takes_value: true,
        multiple: false,
        possible_values: &[],
    }
)]
#[case::fetch_allow_scheme(
    "fetch_allow_scheme",
    FlagShapeExpectation {
        long: "fetch-allow-scheme",
        value_name: Some("SCHEME"),
        takes_value: true,
        multiple: true,
        possible_values: &[],
    }
)]
#[case::json(
    "json",
    FlagShapeExpectation {
        long: "json",
        value_name: None,
        takes_value: false,
        multiple: false,
        possible_values: &[],
    }
)]
fn documented_flags_match_the_parser(
    #[case] field_name: &str,
    #[case] expected: FlagShapeExpectation,
) -> Result<()> {
    let cli = documented_cli(field_name)?;
    let possible_values = cli
        .possible_values
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();

    ensure!(
        cli.long.as_deref() == Some(expected.long),
        "{field_name} should be documented as --{}: {cli:?}",
        expected.long
    );
    ensure!(
        cli.short.is_none(),
        "{field_name} has no short flag in the parser, so release help must not \
         invent one"
    );
    ensure!(
        cli.value_name.as_deref() == expected.value_name,
        "{field_name} should document the parser value name: expected \
         {expected:?}, documented {documented:?}",
        expected = expected.value_name,
        documented = cli.value_name
    );
    ensure!(
        cli.takes_value == expected.takes_value,
        "{field_name} should document whether the parser takes a value"
    );
    ensure!(
        !cli.value_optional,
        "{field_name} takes a required value; the optional-value spelling would \
         advertise a form the parser rejects"
    );
    ensure!(
        cli.multiple == expected.multiple,
        "{field_name} should document whether the parser repeats the flag"
    );
    ensure!(
        possible_values == expected.possible_values,
        "{field_name} should document the accepted values in parser order: \
         expected {expected:?}, documented {possible_values:?}",
        expected = expected.possible_values,
    );
    ensure!(
        !cli.hide_in_help,
        "{field_name} is visible in --help, so the manual page should list it"
    );
    Ok(())
}
