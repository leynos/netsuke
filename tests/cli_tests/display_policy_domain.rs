//! Single-policy CLI accessors independent of the consolidated plan.

use anyhow::{Result, ensure};
use netsuke::cli::Cli;
use netsuke::cli::config::{AccessibilityPolicy, EmojiPolicy, ProgressPolicy};
use netsuke::theme::ThemePreference;
use rstest::rstest;

#[rstest]
#[case(EmojiPolicy::Auto, None)]
#[case(EmojiPolicy::Always, Some(ThemePreference::Unicode))]
#[case(EmojiPolicy::Never, Some(ThemePreference::Ascii))]
fn emoji_policy_maps_to_theme_preference(
    #[case] emoji: EmojiPolicy,
    #[case] expected: Option<ThemePreference>,
) -> Result<()> {
    let cli = Cli {
        emoji,
        ..Cli::default()
    };
    ensure!(
        cli.theme_preference() == expected,
        "emoji {emoji:?} should map to theme {expected:?}, got {:?}",
        cli.theme_preference()
    );
    Ok(())
}

#[rstest]
#[case(AccessibilityPolicy::Auto, None)]
#[case(AccessibilityPolicy::On, Some(true))]
#[case(AccessibilityPolicy::Off, Some(false))]
fn accessibility_policy_maps_to_override(
    #[case] accessibility: AccessibilityPolicy,
    #[case] expected: Option<bool>,
) -> Result<()> {
    let cli = Cli {
        accessibility,
        ..Cli::default()
    };
    ensure!(
        cli.accessibility_override() == expected,
        "accessibility {accessibility:?} should map to override {expected:?}, got {:?}",
        cli.accessibility_override()
    );
    Ok(())
}

#[rstest]
#[case(ProgressPolicy::Auto, true)]
#[case(ProgressPolicy::Always, true)]
#[case(ProgressPolicy::Never, false)]
fn progress_policy_enables_progress(
    #[case] progress: ProgressPolicy,
    #[case] expected: bool,
) -> Result<()> {
    let cli = Cli {
        progress,
        ..Cli::default()
    };
    ensure!(
        cli.progress_enabled() == expected,
        "progress {progress:?} should enable {expected}, got {}",
        cli.progress_enabled()
    );
    Ok(())
}
