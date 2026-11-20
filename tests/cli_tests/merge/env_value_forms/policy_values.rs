//! Case-insensitive policy spellings at the full-merge boundary.

use super::*;
use netsuke::cli::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
use rstest::rstest;

/// Assert every policy carries the variant its mixed-case spelling names.
fn assert_mixed_case_policies(merged: &Cli) -> Result<()> {
    ensure!(
        merged.color == ColourPolicy::Always,
        "mixed-case color should merge as always, got {:?}",
        merged.color,
    );
    ensure!(
        merged.emoji == EmojiPolicy::Always,
        "mixed-case emoji should merge as always, got {:?}",
        merged.emoji,
    );
    ensure!(
        merged.progress == ProgressPolicy::Always,
        "mixed-case progress should merge as always, got {:?}",
        merged.progress,
    );
    ensure!(
        merged.accessibility == AccessibilityPolicy::On,
        "mixed-case accessibility should merge as on, got {:?}",
        merged.accessibility,
    );
    Ok(())
}

/// Mixed-case policy values merge from a configuration file.
#[test]
fn configuration_file_accepts_mixed_case_policies() -> Result<()> {
    let merged = merge(&MergeCase::file(concat!(
        "color = \"AlWaYs\"\n",
        "emoji = \"ALWAYS\"\n",
        "progress = \"aLwAyS\"\n",
        "accessibility = \"oN\"\n",
    )))?;
    assert_mixed_case_policies(&merged)
}

/// Mixed-case policy values merge from the environment.
#[test]
fn environment_accepts_mixed_case_policies() -> Result<()> {
    let merged = merge(&MergeCase::environment(&[
        ("NETSUKE_COLOR", "aLwAyS"),
        ("NETSUKE_EMOJI", "aLwAyS"),
        ("NETSUKE_PROGRESS", "ALWAYS"),
        ("NETSUKE_ACCESSIBILITY", "On"),
    ]))?;
    assert_mixed_case_policies(&merged)
}

/// The environment layer wins over the file layer regardless of casing.
///
/// A valid environment value also masks an invalid file value: the merge
/// deserializes the masked JSON once, so the file's `bogus` never reaches the
/// policy deserializer. That layering contract is why `--json` may
/// short-circuit the early resolver.
#[rstest]
#[case::overrides_valid_file("progress = \"never\"\n")]
#[case::masks_invalid_file("progress = \"bogus\"\n")]
fn environment_policy_wins_over_file_policy(#[case] file: &str) -> Result<()> {
    let merged = merge(&MergeCase::file(file).with_overrides(&[("NETSUKE_PROGRESS", "ALWAYS")]))?;
    ensure!(
        merged.progress == ProgressPolicy::Always,
        "the environment's ALWAYS should win over the file's value, got {:?}",
        merged.progress,
    );
    Ok(())
}

/// An explicit command-line policy wins over the environment.
#[test]
fn command_line_policy_overrides_environment_policy() -> Result<()> {
    let merged = merge(
        &MergeCase::environment(&[("NETSUKE_PROGRESS", "NEVER")]).with_cli(&[
            "netsuke",
            "--progress",
            "always",
        ]),
    )?;
    ensure!(
        merged.progress == ProgressPolicy::Always,
        "the command line's always should override the environment's NEVER, got {:?}",
        merged.progress,
    );
    Ok(())
}

/// An invalid winning value is rejected with the policy's error text.
#[test]
fn invalid_environment_policy_over_file_policy_is_rejected() -> Result<()> {
    let error = merge_error(
        &MergeCase::file("progress = \"always\"\n")
            .with_overrides(&[("NETSUKE_PROGRESS", "bogus")]),
    )?;
    ensure!(
        error.contains("invalid progress policy 'bogus'"),
        "the invalid environment policy should be reported, got: {error}",
    );
    ensure!(
        error.contains("expected one of: auto, always, never"),
        "the rejection should name the accepted spellings, got: {error}",
    );
    Ok(())
}

/// The environment value parser trims surrounding whitespace before parsing.
///
/// Environment values are unquoted scalars, so Figment's text parser trims
/// them. A configuration file keeps the string literal verbatim, which is why
/// the same spelling is rejected there; see the companion test below.
#[test]
fn surrounding_whitespace_in_environment_policy_is_trimmed() -> Result<()> {
    let merged = merge(&MergeCase::environment(&[("NETSUKE_COLOR", " AlWaYs ")]))?;
    ensure!(
        merged.color == ColourPolicy::Always,
        "the environment parser should trim and accept ' AlWaYs ', got {:?}",
        merged.color,
    );
    Ok(())
}

/// A configuration-file policy string keeps its surrounding whitespace.
#[test]
fn surrounding_whitespace_in_file_policy_is_rejected() -> Result<()> {
    let error = merge_error(&MergeCase::file("color = \" AlWaYs \"\n"))?;
    ensure!(
        error.contains("invalid color policy ' AlWaYs '"),
        "a quoted file value keeps its whitespace, so it should be rejected, got: {error}",
    );
    Ok(())
}
