//! Case-insensitive policy spellings at the full-merge boundary.

use super::*;
use netsuke::cli::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};

/// Mixed-case policy values merge from a configuration file.
#[test]
fn configuration_file_accepts_mixed_case_policies() -> Result<()> {
    let merged = merge_with_file_and_environment(
        concat!(
            "color = \"AlWaYs\"\n",
            "emoji = \"ALWAYS\"\n",
            "progress = \"nEvEr\"\n",
            "accessibility = \"oN\"\n",
        ),
        &[],
        &["netsuke"],
    )?;
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
        merged.progress == ProgressPolicy::Never,
        "mixed-case progress should merge as never, got {:?}",
        merged.progress,
    );
    ensure!(
        merged.accessibility == AccessibilityPolicy::On,
        "mixed-case accessibility should merge as on, got {:?}",
        merged.accessibility,
    );
    Ok(())
}

/// Mixed-case policy values merge from the environment.
#[test]
fn environment_accepts_mixed_case_policies() -> Result<()> {
    let merged = merge_with_environment(
        &[
            ("NETSUKE_COLOR", "AlWaYs"),
            ("NETSUKE_EMOJI", "aLwAyS"),
            ("NETSUKE_PROGRESS", "ALWAYS"),
            ("NETSUKE_ACCESSIBILITY", "On"),
        ],
        &["netsuke"],
    )?;
    ensure!(
        merged.color == ColourPolicy::Always,
        "mixed-case NETSUKE_COLOR should merge as always, got {:?}",
        merged.color,
    );
    ensure!(
        merged.emoji == EmojiPolicy::Always,
        "mixed-case NETSUKE_EMOJI should merge as always, got {:?}",
        merged.emoji,
    );
    ensure!(
        merged.progress == ProgressPolicy::Always,
        "mixed-case NETSUKE_PROGRESS should merge as always, got {:?}",
        merged.progress,
    );
    ensure!(
        merged.accessibility == AccessibilityPolicy::On,
        "mixed-case NETSUKE_ACCESSIBILITY should merge as on, got {:?}",
        merged.accessibility,
    );
    Ok(())
}

/// The environment layer wins over the file layer regardless of casing.
#[test]
fn environment_policy_overrides_file_policy() -> Result<()> {
    let merged = merge_with_file_and_environment(
        "progress = \"never\"\n",
        &[("NETSUKE_PROGRESS", "ALWAYS")],
        &["netsuke"],
    )?;
    ensure!(
        merged.progress == ProgressPolicy::Always,
        "the environment's ALWAYS should override the file's never, got {:?}",
        merged.progress,
    );
    Ok(())
}

/// An explicit command-line policy wins over the environment.
#[test]
fn command_line_policy_overrides_environment_policy() -> Result<()> {
    let merged = merge_with_environment(
        &[("NETSUKE_PROGRESS", "NEVER")],
        &["netsuke", "--progress", "always"],
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
    let error = merge_with_file_and_environment_error(
        "progress = \"always\"\n",
        &[("NETSUKE_PROGRESS", "bogus")],
        &["netsuke"],
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

/// A valid higher layer masks an invalid lower-layer policy value.
///
/// The merge deserializes the masked JSON once, so the file's `bogus` value
/// never reaches the policy deserializer. This is the documented layering
/// contract, and it is why `--json` may short-circuit the early resolver.
#[test]
fn valid_environment_policy_masks_invalid_file_policy() -> Result<()> {
    let merged = merge_with_file_and_environment(
        "progress = \"bogus\"\n",
        &[("NETSUKE_PROGRESS", "ALWAYS")],
        &["netsuke"],
    )?;
    ensure!(
        merged.progress == ProgressPolicy::Always,
        "the valid environment policy should mask the invalid file value, got {:?}",
        merged.progress,
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
    let merged = merge_with_environment(&[("NETSUKE_COLOR", " AlWaYs ")], &["netsuke"])?;
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
    let error = merge_with_file_and_environment_error("color = \" AlWaYs \"\n", &[], &["netsuke"])?;
    ensure!(
        error.contains("invalid color policy ' AlWaYs '"),
        "a quoted file value keeps its whitespace, so it should be rejected, got: {error}",
    );
    Ok(())
}
