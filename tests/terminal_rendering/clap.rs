//! Check Clap's help and usage rendering against the startup colour policy.
//!
//! Only the pipe cases live here. The pseudo-terminal cases need the Unix-only
//! PTY harness and are in `clap_pty`, because this module is compiled on every
//! platform and naming a Unix-gated item in an ungated module fails the
//! Windows build.

use super::support::{TerminalWorkspace, display_plan_for_args};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use pretty_assertions::assert_eq;
use rstest::rstest;
use strip_ansi_escapes::strip;
use test_support::terminal::classify;

/// The three command forms that render clap help.
///
/// `--help` exits through clap's own renderer. `help` and `help build` go
/// through the runner's renderers, which build the same `Command` themselves
/// and so take their colour from the resolved display plan instead of from
/// clap's `ColorChoice`. Both routes must reach the same verdict.
#[derive(Clone, Copy)]
enum HelpForm {
    /// Clap's own `--help` switch.
    Flag,
    /// The `help` subcommand with no topic.
    Root,
    /// The `help build` subcommand topic.
    Subcommand,
}

impl HelpForm {
    /// Return the argv that requests this help form after `colour_args`.
    fn argv(self) -> &'static [&'static str] {
        match self {
            Self::Flag => &["--help"],
            Self::Root => &["help"],
            Self::Subcommand => &["help", "build"],
        }
    }
}

/// Help-form rows reused by the pipe cases below.
const HELP_FORMS: [HelpForm; 3] = [HelpForm::Flag, HelpForm::Root, HelpForm::Subcommand];

/// Pin Clap help colour for a piped stdout across all three renderers.
///
/// A pipe is the plainest transport: `auto` resolves off, `always` overrides
/// it, and `never` plus `NO_COLOR` suppress it. Running every row against every
/// help form is what keeps the runner-rendered paths from silently drifting
/// away from clap's own.
#[googletest::gtest]
#[rstest]
#[case::cli_never(&["--color", "never"], &[])]
#[case::cli_always(&["--color", "always"], &[])]
#[case::cli_auto(&["--color", "auto"], &[])]
#[case::no_color(&[], &[("NO_COLOR", "1")])]
#[case::environment_always(&[], &[("NETSUKE_COLOR", "always")])]
#[case::cli_overrides_environment(&["--color", "never"], &[("NETSUKE_COLOR", "always")])]
fn piped_help_colour_matches_the_startup_policy(
    #[case] colour_args: &[&str],
    #[case] extra_env: &[(&str, &str)],
) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let plan = display_plan_for_args(colour_args, extra_env, false, false)?;
    let expected_colour = plan.stdout_colour();

    for form in HELP_FORMS {
        let args = colour_args
            .iter()
            .chain(form.argv().iter())
            .copied()
            .collect::<Vec<_>>();
        let output = workspace
            .run_piped(&args, extra_env)
            .with_context(|| format!("run piped help form {args:?}"))?;
        expect_that!(output.status.success(), eq(true));
        expect_that!(classify(&output.stdout).has_sgr, eq(expected_colour));
        expect_that!(
            String::from_utf8_lossy(&output.stdout),
            contains_substring("Usage")
        );
    }
    Ok(())
}

/// Keep invalid-argument usage diagnostics plain even under forced colour.
///
/// Clap rebuilds localized usage errors through `ortho_config`'s
/// `ClapError::raw`, which pins their colour choice to `Never`. That is a
/// documented decision (`D9`) rather than an oversight, so it is asserted
/// rather than left to drift.
#[googletest::gtest]
#[test]
fn usage_error_remains_plain_when_colour_is_forced() -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let output = workspace
        .run_piped(&["--color", "always", "unknown-command"], &[])
        .context("run an invalid command with forced colour")?;
    ensure!(
        !output.status.success(),
        "an invalid command should fail before execution"
    );
    expect_that!(classify(&output.stderr).has_sgr, eq(false));
    expect_that!(
        String::from_utf8_lossy(&output.stderr),
        contains_substring("Usage")
    );
    Ok(())
}

/// Colour must add emphasis to help text without carrying meaning.
///
/// `INV-COLOUR-ADDITIVE` (`UG-NO-COLOUR-ALONE`) for the help emitters:
/// stripping the SGR sequences from `--color always` help must reproduce the
/// `--color never` help byte for byte. Help is otherwise a fixed output, so any
/// difference beyond styling is a content change hiding behind a colour policy.
/// Non-vacuity is asserted first, because two plain streams would compare equal
/// while testing nothing.
#[googletest::gtest]
#[rstest]
#[case::root(&["help"])]
#[case::subcommand(&["help", "build"])]
fn stripping_colour_from_help_reproduces_the_plain_help(#[case] help_args: &[&str]) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    let coloured_args = [&["--color", "always"], help_args].concat();
    let plain_args = [&["--color", "never"], help_args].concat();

    let coloured = workspace
        .run_piped(&coloured_args, &[])
        .context("run colour-on help")?;
    let plain = workspace
        .run_piped(&plain_args, &[])
        .context("run colour-off help")?;
    ensure!(
        coloured.status.success() && plain.status.success(),
        "both help forms should exit successfully"
    );

    ensure!(
        classify(&coloured.stdout).has_sgr,
        "the colour-on help must contain SGR for the comparison to be meaningful"
    );
    expect_that!(classify(&plain.stdout).has_sgr, eq(false));

    let stripped =
        String::from_utf8(strip(&coloured.stdout)).context("stripped help output is not UTF-8")?;
    let expected =
        String::from_utf8(plain.stdout.clone()).context("colour-off help output is not UTF-8")?;
    assert_eq!(stripped, expected);
    Ok(())
}
