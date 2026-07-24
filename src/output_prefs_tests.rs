//! Tests for output preference resolution and token-backed prefix rendering.

use super::*;
use crate::snapshot_test_support::{NoColorEnv, no_color_env, snapshot_settings, theme_prefs};
use anyhow::{Result, ensure};
use insta::assert_snapshot;
use rstest::rstest;
use test_support::fluent::normalize_fluent_isolates;
use test_support::{EnLocalizer, en_localizer};

#[derive(Debug)]
struct ThemeResolutionCase<'a> {
    theme: Option<ThemePreference>,
    no_emoji: Option<bool>,
    mode: OutputMode,
    no_color: Option<&'a str>,
    expected_emoji: bool,
}

#[rstest]
#[case::explicit_no_emoji_forces_ascii(Some(true), None, false)]
#[case::false_defers_to_no_color(Some(false), Some("1"), false)]
#[case::explicit_false_allows_unicode(Some(false), None, true)]
#[case::no_color_disables_emoji(None, Some("1"), false)]
#[case::no_color_empty_disables_emoji(None, Some(""), false)]
#[case::default_allows_unicode(None, None, true)]
fn resolve_output_prefs(
    no_color_env: NoColorEnv,
    #[case] no_emoji: Option<bool>,
    #[case] no_color: Option<&str>,
    #[case] expected_emoji: bool,
) {
    assert_eq!(
        resolve_with(no_emoji, move |key| {
            no_color_env(no_color.map(String::from), key)
        })
        .emoji_allowed(),
        expected_emoji
    );
}

#[rstest]
#[case::unicode_theme_is_explicit(ThemeResolutionCase {
    theme: Some(ThemePreference::Unicode),
    no_emoji: None,
    mode: OutputMode::Standard,
    no_color: None,
    expected_emoji: true,
})]
#[case::ascii_theme_stays_ascii_without_env(ThemeResolutionCase {
    theme: Some(ThemePreference::Ascii),
    no_emoji: None,
    mode: OutputMode::Standard,
    no_color: None,
    expected_emoji: false,
})]
#[case::auto_theme_no_color_forces_ascii(ThemeResolutionCase {
    theme: Some(ThemePreference::Auto),
    no_emoji: None,
    mode: OutputMode::Standard,
    no_color: Some("1"),
    expected_emoji: false,
})]
#[case::auto_theme_standard_without_env_uses_unicode(ThemeResolutionCase {
    theme: Some(ThemePreference::Auto),
    no_emoji: None,
    mode: OutputMode::Standard,
    no_color: None,
    expected_emoji: true,
})]
#[case::auto_theme_legacy_no_emoji_stays_ascii(ThemeResolutionCase {
    theme: Some(ThemePreference::Auto),
    no_emoji: Some(true),
    mode: OutputMode::Standard,
    no_color: None,
    expected_emoji: false,
})]
fn resolve_from_theme_with_uses_theme_resolution(
    no_color_env: NoColorEnv,
    #[case] case: ThemeResolutionCase<'_>,
) {
    let prefs = resolve_from_theme_with(
        case.theme,
        ThemeContext::new(case.no_emoji, None, case.mode),
        move |key| no_color_env(case.no_color.map(String::from), key),
    );
    assert_eq!(prefs.emoji_allowed(), case.expected_emoji);
}

#[rstest]
#[case::unicode_error(Some(ThemePreference::Unicode), OutputPrefs::error_prefix, "✖ Error:")]
#[case::ascii_error(Some(ThemePreference::Ascii), OutputPrefs::error_prefix, "X Error:")]
#[case::unicode_success(
    Some(ThemePreference::Unicode),
    OutputPrefs::success_prefix,
    "✔ Success:"
)]
#[case::ascii_success(
    Some(ThemePreference::Ascii),
    OutputPrefs::success_prefix,
    "+ Success:"
)]
#[case::unicode_warning(
    Some(ThemePreference::Unicode),
    OutputPrefs::warning_prefix,
    "⚠ Warning:"
)]
#[case::ascii_warning(
    Some(ThemePreference::Ascii),
    OutputPrefs::warning_prefix,
    "! Warning:"
)]
#[case::unicode_info(Some(ThemePreference::Unicode), OutputPrefs::info_prefix, "ℹ Info:")]
#[case::ascii_info(Some(ThemePreference::Ascii), OutputPrefs::info_prefix, "i Info:")]
#[case::unicode_timing(
    Some(ThemePreference::Unicode),
    OutputPrefs::timing_prefix,
    "⏱ Timing:"
)]
#[case::ascii_timing(Some(ThemePreference::Ascii), OutputPrefs::timing_prefix, "T Timing:")]
fn prefix_rendering_uses_theme_symbols(
    en_localizer: EnLocalizer,
    #[case] theme: Option<ThemePreference>,
    #[case] prefix_fn: fn(OutputPrefs) -> String,
    #[case] expected: &str,
) -> Result<()> {
    let _localizer = en_localizer;
    let prefs = resolve_from_theme_with(
        theme,
        ThemeContext::new(None, None, OutputMode::Standard),
        |_| None,
    );
    ensure!(
        prefix_fn(prefs) == expected,
        "prefix output should match the pinned en-US expectation"
    );
    Ok(())
}

#[rstest]
#[case::accessible_auto(OutputMode::Accessible)]
#[case::standard_ascii(OutputMode::Standard)]
fn spacing_accessors_follow_resolved_theme(#[case] mode: OutputMode) {
    let prefs = resolve_from_theme_with(
        Some(ThemePreference::Auto),
        ThemeContext::new(Some(true), None, mode),
        |_| None,
    );
    assert_eq!(prefs.task_indent(), "  ");
    assert_eq!(prefs.timing_indent(), "  ");
}

#[rstest]
#[case::unicode(crate::theme::ThemePreference::Unicode, "all_prefixes_unicode")]
#[case::ascii(crate::theme::ThemePreference::Ascii, "all_prefixes_ascii")]
fn prefix_and_spacing_snapshot(
    en_localizer: EnLocalizer,
    #[case] theme: crate::theme::ThemePreference,
    #[case] snapshot_name: &str,
) {
    let _localizer = en_localizer;
    let prefs = theme_prefs(theme);
    let rendered = normalize_fluent_isolates(&format!(
        concat!(
            "error_prefix:   {}\n",
            "warning_prefix: {}\n",
            "success_prefix: {}\n",
            "info_prefix:    {}\n",
            "timing_prefix:  {}\n",
            "task_indent:    {:?}\n",
            "timing_indent:  {:?}"
        ),
        prefs.error_prefix(),
        prefs.warning_prefix(),
        prefs.success_prefix(),
        prefs.info_prefix(),
        prefs.timing_prefix(),
        prefs.task_indent(),
        prefs.timing_indent(),
    ));

    snapshot_settings("output_prefs").bind(|| {
        assert_snapshot!(snapshot_name, rendered);
    });
}
