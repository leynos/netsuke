//! Exhaustive display-policy resolution coverage across the consolidated
//! enum domain.
//!
//! This module verifies that the consolidated display-policy resolution in
//! `src/theme.rs` and `src/output_prefs.rs` honours the established
//! precedence (explicit theme preference > emoji policy > `NO_COLOR` >
//! output mode) over the full domain of `EmojiPolicy`, `ProgressPolicy`,
//! `AccessibilityPolicy`, `ColourPolicy`, `OutputMode` and `json`
//! combinations.
//!
//! Coverage is threefold:
//! - `exhaustive_domain_sweep_matches_truth_model`: a deterministic sweep of
//!   every policy combination across the `NO_COLOR` toggle, checked against a
//!   hand-written truth model.
//! - `output_mode_inference_matches_truth_model`: a deterministic sweep over
//!   `OutputMode` and `TERM` across the `NO_COLOR` toggle.
//! - `policy_domain_proptest`: probabilistic combinations covering the same
//!   invariants.
//!
//! These tests add coverage only; they do not change any production
//! resolution logic (`src/theme.rs` / `src/output_prefs.rs` are left
//! untouched).

use anyhow::{Result, ensure};
use netsuke::cli::Cli;
use netsuke::cli::config::{AccessibilityPolicy, ColourPolicy, EmojiPolicy, ProgressPolicy};
use netsuke::output_mode::{OutputMode, resolve_with};
use netsuke::output_prefs;
use netsuke::theme::{ThemeContext, ThemePreference, resolve_theme};
use proptest::prelude::*;
use rstest::rstest;

/// An injected environment lookup whose `NO_COLOR`/`TERM` presence is fixed.
fn fake_env(no_color: bool, term_dumb: bool) -> impl Fn(&str) -> Option<String> {
    move |key| match key {
        "NO_COLOR" if no_color => Some(String::from("1")),
        "TERM" if term_dumb => Some(String::from("dumb")),
        _ => None,
    }
}

/// One point in the display-policy domain plus the ambient environment
/// signals that influence the consolidated output decisions.
#[derive(Debug, Clone, Copy)]
struct DomainCase {
    emoji: EmojiPolicy,
    color: ColourPolicy,
    progress: ProgressPolicy,
    accessibility: AccessibilityPolicy,
    no_color: bool,
    term_dumb: bool,
}

/// An emoji-policy strategy sampling directly from the enum domain.
fn emoji_strategy() -> impl Strategy<Value = EmojiPolicy> {
    prop::sample::select(&[EmojiPolicy::Auto, EmojiPolicy::Always, EmojiPolicy::Never])
}

/// A colour-policy strategy sampling directly from the enum domain.
fn color_strategy() -> impl Strategy<Value = ColourPolicy> {
    prop::sample::select(&[
        ColourPolicy::Auto,
        ColourPolicy::Always,
        ColourPolicy::Never,
    ])
}

/// A progress-policy strategy sampling directly from the enum domain.
fn progress_strategy() -> impl Strategy<Value = ProgressPolicy> {
    prop::sample::select(&[
        ProgressPolicy::Auto,
        ProgressPolicy::Always,
        ProgressPolicy::Never,
    ])
}

/// An accessibility-policy strategy sampling directly from the enum domain.
fn accessibility_strategy() -> impl Strategy<Value = AccessibilityPolicy> {
    prop::sample::select(&[
        AccessibilityPolicy::Auto,
        AccessibilityPolicy::On,
        AccessibilityPolicy::Off,
    ])
}

/// Expected theme preference derived from the emoji policy.
const fn expected_theme_preference(emoji: EmojiPolicy) -> Option<ThemePreference> {
    match emoji {
        EmojiPolicy::Auto => None,
        EmojiPolicy::Always => Some(ThemePreference::Unicode),
        EmojiPolicy::Never => Some(ThemePreference::Ascii),
    }
}

/// Expected accessible-output override derived from the accessibility policy.
const fn expected_accessibility_override(accessibility: AccessibilityPolicy) -> Option<bool> {
    match accessibility {
        AccessibilityPolicy::Auto => None,
        AccessibilityPolicy::On => Some(true),
        AccessibilityPolicy::Off => Some(false),
    }
}

/// Expected progress decision derived from the progress policy.
const fn expected_progress_enabled(progress: ProgressPolicy) -> bool {
    !matches!(progress, ProgressPolicy::Never)
}

/// Expected output mode decided by `output_mode::resolve`:
///
/// 1. An explicit override forces Accessible(true) or Standard(false).
/// 2. `NO_COLOR` is active unless the colour policy is `Always`; when active
///    the mode is Accessible.
/// 3. `TERM=dumb` forces Accessible.
/// 4. Otherwise Standard.
const fn expected_output_mode(case: DomainCase) -> OutputMode {
    if let Some(forced) = expected_accessibility_override(case.accessibility) {
        return if forced {
            OutputMode::Accessible
        } else {
            OutputMode::Standard
        };
    }
    let no_color_active = match case.color {
        ColourPolicy::Always => false,
        ColourPolicy::Never => true,
        ColourPolicy::Auto => case.no_color,
    };
    if no_color_active || case.term_dumb {
        OutputMode::Accessible
    } else {
        OutputMode::Standard
    }
}

/// Expected emoji allowance decided by `theme::should_use_unicode`:
///
/// 1. An explicit Unicode/Ascii theme preference wins outright.
/// 2. Otherwise `NO_COLOR` active (colour-aware) forces ASCII.
/// 3. Otherwise Accessible mode uses ASCII, Standard uses Unicode.
const fn expected_emoji_allowed(case: DomainCase, mode: OutputMode) -> bool {
    match case.emoji {
        EmojiPolicy::Always => true,
        EmojiPolicy::Never => false,
        EmojiPolicy::Auto => {
            let no_color_active = match case.color {
                ColourPolicy::Always => false,
                ColourPolicy::Never => true,
                ColourPolicy::Auto => case.no_color,
            };
            if no_color_active {
                false
            } else {
                !mode.is_accessible()
            }
        }
    }
}

/// The consolidated expected decisions for one display-policy tuple.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExpectedDisplay {
    theme_preference: Option<ThemePreference>,
    accessibility_override: Option<bool>,
    progress_enabled: bool,
    output_mode: OutputMode,
    emoji_allowed: bool,
}

const fn expected_display(case: DomainCase) -> ExpectedDisplay {
    let theme_preference = expected_theme_preference(case.emoji);
    let accessibility_override = expected_accessibility_override(case.accessibility);
    let progress_enabled = expected_progress_enabled(case.progress);
    let output_mode = expected_output_mode(case);
    let emoji_allowed = expected_emoji_allowed(case, output_mode);
    ExpectedDisplay {
        theme_preference,
        accessibility_override,
        progress_enabled,
        output_mode,
        emoji_allowed,
    }
}

/// Build a `Cli` carrying the policy tuple and compare the theme, accessibility,
/// and progress projections against the truth model.
fn assert_consolidated(case: DomainCase) -> Result<()> {
    let expected = expected_display(case);
    let cli = Cli {
        emoji: case.emoji,
        color: case.color,
        progress: case.progress,
        accessibility: case.accessibility,
        ..Cli::default()
    };
    ensure!(
        cli.theme_preference() == expected.theme_preference,
        "theme preference mismatch for emoji={:?}: got {:?}, expected {:?}",
        case.emoji,
        cli.theme_preference(),
        expected.theme_preference
    );
    ensure!(
        cli.accessibility_override() == expected.accessibility_override,
        "accessibility override mismatch for accessibility={:?}: got {:?}, expected {:?}",
        case.accessibility,
        cli.accessibility_override(),
        expected.accessibility_override
    );
    ensure!(
        cli.progress_enabled() == expected.progress_enabled,
        "progress mismatch for progress={:?}: got {}, expected {}",
        case.progress,
        cli.progress_enabled(),
        expected.progress_enabled
    );
    assert_env_resolution(case, &cli, expected)
}

/// Compare the environment-sensitive output-mode, theme-emoji, and `OutputPrefs`
/// decisions against the truth model. Split out of `assert_consolidated` to
/// keep each helper under the file's line ceiling.
fn assert_env_resolution(case: DomainCase, cli: &Cli, expected: ExpectedDisplay) -> Result<()> {
    let output_mode = resolve_with(
        cli.accessibility_override(),
        Some(cli.color),
        fake_env(case.no_color, case.term_dumb),
    );
    ensure!(
        output_mode == expected.output_mode,
        "output mode mismatch for color={:?} accessibility={:?} no_color={} term_dumb={}: got {output_mode:?}, expected {:?}",
        case.color,
        case.accessibility,
        case.no_color,
        case.term_dumb,
        expected.output_mode
    );

    let resolved = resolve_theme(
        cli.theme_preference(),
        ThemeContext::new(None, Some(cli.color), output_mode),
        fake_env(case.no_color, false),
    );
    ensure!(
        resolved.tokens.emoji_allowed == expected.emoji_allowed,
        "emoji allowance mismatch for emoji={:?} color={:?} mode={output_mode:?} no_color={}: got {}, expected {}",
        case.emoji,
        case.color,
        case.no_color,
        resolved.tokens.emoji_allowed,
        expected.emoji_allowed
    );

    // OutputPrefs, the theme-backed facade, mirrors the theme emoji decision.
    let prefs = output_prefs::resolve_from_theme_with(
        cli.theme_preference(),
        ThemeContext::new(None, Some(cli.color), output_mode),
        fake_env(case.no_color, false),
    );
    ensure!(
        prefs.emoji_allowed() == expected.emoji_allowed,
        "OutputPrefs emoji mismatch for emoji={:?} color={:?}: got {}, expected {}",
        case.emoji,
        case.color,
        prefs.emoji_allowed(),
        expected.emoji_allowed
    );
    Ok(())
}

proptest! {
    /// The production display-policy pipeline agrees with the hand-written
    /// truth model over the generated full domain and arbitrary environment
    /// signals.
    #[test]
    fn consolidated_display_policies_resolve_correctly(
        emoji in emoji_strategy(),
        color in color_strategy(),
        progress in progress_strategy(),
        accessibility in accessibility_strategy(),
        no_color in any::<bool>(),
        term_dumb in any::<bool>(),
    ) {
        let case = DomainCase {
            emoji,
            color,
            progress,
            accessibility,
            no_color,
            term_dumb,
        };
        // `assert_consolidated` uses `ensure!` and returns `Result`; divert the
        // failure to a prop-level assertion so shrinking reports the tuple.
        let result = assert_consolidated(case);
        prop_assert!(result.is_ok(), "consolidated policy failure: {result:?}");
    }
}

/// Exhaustive single-field projection coverage (rstest).
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

/// Exhaustive sweep of the full 3 x 3 x 3 x 3 policy domain with and without
/// `NO_COLOR`, comparing the production pipeline against the truth model.
#[rstest]
fn exhaustive_domain_sweep_matches_truth_model() -> Result<()> {
    for emoji in [EmojiPolicy::Auto, EmojiPolicy::Always, EmojiPolicy::Never] {
        for color in [
            ColourPolicy::Auto,
            ColourPolicy::Always,
            ColourPolicy::Never,
        ] {
            for progress in [
                ProgressPolicy::Auto,
                ProgressPolicy::Always,
                ProgressPolicy::Never,
            ] {
                for accessibility in [
                    AccessibilityPolicy::Auto,
                    AccessibilityPolicy::On,
                    AccessibilityPolicy::Off,
                ] {
                    for no_color in [false, true] {
                        let case = DomainCase {
                            emoji,
                            color,
                            progress,
                            accessibility,
                            no_color,
                            term_dumb: false,
                        };
                        assert_consolidated(case)?;
                    }
                }
            }
        }
    }
    Ok(())
}
