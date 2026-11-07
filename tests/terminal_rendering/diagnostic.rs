//! Check that Miette diagnostics follow the resolved colour and glyph policy.
//!
//! Only the pipe cases live here. The pseudo-terminal cases need the Unix-only
//! PTY harness and are in `diagnostic_pty`, because this module is compiled on
//! every platform and naming a Unix-gated item in an ungated module fails the
//! Windows build.

use super::support::{display_plan_for_args, empty_workspace};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use pretty_assertions::assert_eq;
use strip_ansi_escapes::strip;
use test_support::terminal::classify;

/// Check that forced colour styles a diagnostic written to a pipe.
#[googletest::gtest]
#[test]
fn piped_diagnostic_colour_matches_the_display_plan() -> Result<()> {
    let workspace = empty_workspace()?;
    let args = ["--color", "always", "generate"];
    let plan = display_plan_for_args(&args, &[], false, false)?;
    let output = workspace
        .run_piped(&args, &[])
        .context("run piped missing-manifest diagnostic")?;
    ensure!(!output.status.success(), "missing manifest should fail");

    expect_that!(plan.diagnostic_style().colour, eq(true));
    expect_that!(classify(&output.stderr).has_sgr, eq(true));
    Ok(())
}

/// Colour must add emphasis without carrying meaning.
///
/// `INV-COLOUR-ADDITIVE` (`UG-NO-COLOUR-ALONE`): stripping the SGR sequences
/// from a colour-on diagnostic must reproduce the colour-off diagnostic byte
/// for byte. If it does not, colour is encoding something a monochrome reader
/// cannot see. Non-vacuity is asserted first: a colour-on stream with no SGR
/// would satisfy the comparison trivially.
///
/// Glyph and accessibility policy are pinned identically on both sides. They
/// are not colour: `--color never` also selects accessible mode when
/// accessibility is `auto`, which swaps every Unicode glyph and every progress
/// line for its ASCII or static counterpart. Letting that drift would compare
/// two differently rendered streams and blame the difference on colour.
#[googletest::gtest]
#[test]
fn stripping_colour_from_a_diagnostic_reproduces_the_plain_diagnostic() -> Result<()> {
    let workspace = empty_workspace()?;
    let pinned = ["--accessibility", "off", "--emoji", "always"];
    let coloured_args = [&["--color", "always"][..], &pinned[..], &["generate"][..]].concat();
    let plain_args = [&["--color", "never"][..], &pinned[..], &["generate"][..]].concat();

    let coloured = workspace
        .run_piped(&coloured_args, &[])
        .context("run colour-on missing-manifest diagnostic")?;
    let plain = workspace
        .run_piped(&plain_args, &[])
        .context("run colour-off missing-manifest diagnostic")?;
    ensure!(
        !coloured.status.success() && !plain.status.success(),
        "the missing-manifest diagnostic should fail on both colour policies"
    );

    // Non-vacuity: the comparison below is only meaningful when the colour-on
    // stream actually carries styling to remove.
    ensure!(
        classify(&coloured.stderr).has_sgr,
        "the colour-on diagnostic must contain SGR for the comparison to be meaningful"
    );
    expect_that!(classify(&plain.stderr).has_sgr, eq(false));

    // Compared as bytes rather than as text: the lossy conversion and the
    // Fluent isolate normalisation both erase distinctions this invariant is
    // meant to catch.
    assert_eq!(strip(&coloured.stderr), plain.stderr);
    Ok(())
}

/// A colour policy set in a configuration file reaches the runtime plan.
///
/// The command path that matters is the merged one: `run_configured_command`
/// builds its plan from the configuration merged with argv, not from the
/// pre-configuration startup signals that every other case in this module
/// exercises. Without a configuration-file case the merged resolver could stop
/// reading `color` and nothing here would notice, because each existing case
/// passes the policy on the command line and would keep resolving correctly
/// through either path.
#[googletest::gtest]
#[test]
fn configuration_file_colour_reaches_the_runtime_plan() -> Result<()> {
    let workspace = empty_workspace()?;
    test_support::fs::write(
        workspace.root_path().join(".netsuke.toml"),
        "color = \"always\"\n",
    )
    .context("write the colour configuration")?;

    let from_configuration = workspace
        .run_piped(&["generate"], &[])
        .context("run the missing-manifest diagnostic under configuration colour")?;
    ensure!(
        !from_configuration.status.success(),
        "the missing-manifest diagnostic should fail"
    );
    // The configuration file is the only source of colour here: no `--color`
    // and no NETSUKE_COLOR, so a green assertion cannot come from argv.
    ensure!(
        classify(&from_configuration.stderr).has_sgr,
        "color = \"always\" in .netsuke.toml must style the diagnostic"
    );

    // The command line still wins over the file, which is the precedence the
    // merged resolver is responsible for rather than the startup one.
    let overridden = workspace
        .run_piped(&["--color", "never", "generate"], &[])
        .context("run the diagnostic with an overriding colour policy")?;
    ensure!(
        !overridden.status.success(),
        "the missing-manifest diagnostic should fail under the override"
    );
    expect_that!(classify(&overridden.stderr).has_sgr, eq(false));
    Ok(())
}

/// The accessible diagnostic format is pinned so a later handler switch shows.
///
/// `D8` defers Miette's narratable (screen-reader) handler to a follow-up and
/// pins the current accessible fingerprint now, so adopting that handler later
/// appears as an explicit diff rather than an unnoticed format change. The
/// fingerprint records the structural shape — the SGR-present flag, the OSC
/// flag, the isolate and non-ASCII flags, and the first line of text — not
/// exact bytes, which would make every cosmetic edit a test failure. Progress
/// is silent here so the first line is the diagnostic's own, not a stage label
/// that a change to the progress reporter would move.
#[googletest::gtest]
#[test]
fn accessible_diagnostic_matches_the_pinned_fingerprint() -> Result<()> {
    let workspace = empty_workspace()?;
    let args = [
        "--color",
        "always",
        "--accessibility",
        "on",
        "--progress",
        "never",
        "generate",
    ];
    let plan = display_plan_for_args(&args, &[], false, false)?;
    let output = workspace
        .run_piped(&args, &[])
        .context("run accessible missing-manifest diagnostic")?;
    ensure!(!output.status.success(), "missing manifest should fail");

    ensure!(
        plan.output_mode().is_accessible(),
        "the fingerprint case must resolve to accessible output"
    );
    let facts = classify(&output.stderr);
    let plain = String::from_utf8(strip(&output.stderr))
        .context("stripped accessible diagnostic is not UTF-8")?;
    let first_line = plain.lines().next().unwrap_or_default().trim_end();

    let fingerprint = format!(
        "colour={} links={} sgr={} osc={} bidi={} non_ascii={} first_line={first_line:?}",
        plan.diagnostic_style().colour,
        plan.diagnostic_style().links,
        facts.has_sgr,
        facts.has_osc,
        facts.has_bidi_isolates,
        facts.has_non_ascii_glyphs,
    );
    expect_that!(
        fingerprint.as_str(),
        eq(concat!(
            "colour=true links=false sgr=true osc=false bidi=true non_ascii=false ",
            "first_line=\"X Error: netsuke::runner::manifest_not_found\""
        ))
    );
    Ok(())
}
