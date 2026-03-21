//! Tests for output preference resolution and token-backed prefix rendering.

use super::*;
use rstest::rstest;

#[derive(Debug)]
struct ThemeResolutionCase<'a> {
    theme: Option<ThemePreference>,
    no_emoji: Option<bool>,
    mode: OutputMode,
    no_color: Option<&'a str>,
    no_emoji_env: Option<&'a str>,
    expected_emoji: bool,
}

/// Build an environment lookup from optional `NO_COLOR` and
/// `NETSUKE_NO_EMOJI` values.
fn fake_env<'a>(
    no_color: Option<&'a str>,
    no_emoji_env: Option<&'a str>,
) -> impl Fn(&str) -> Option<String> + 'a {
    move |key| match key {
        "NO_COLOR" => no_color.map(String::from),
        "NETSUKE_NO_EMOJI" => no_emoji_env.map(String::from),
        _ => None,
    }
}

fn strip_isolates(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(ch, '\u{2068}' | '\u{2069}'))
        .collect()
}

#[rstest]
#[case::explicit_no_emoji_forces_ascii(Some(true), None, None, false)]
#[case::false_defers_to_no_color(Some(false), Some("1"), None, false)]
#[case::false_defers_to_netsuke_no_emoji(Some(false), None, Some("1"), false)]
#[case::no_color_disables_emoji(None, Some("1"), None, false)]
#[case::no_color_empty_disables_emoji(None, Some(""), None, false)]
#[case::netsuke_no_emoji_disables(None, None, Some("1"), false)]
#[case::netsuke_no_emoji_empty_disables(None, None, Some(""), false)]
#[case::netsuke_no_emoji_false_string_disables(None, None, Some("false"), false)]
#[case::netsuke_no_emoji_zero_string_disables(None, None, Some("0"), false)]
#[case::false_defers_to_netsuke_no_emoji_false_string(Some(false), None, Some("false"), false)]
#[case::default_allows_unicode(None, None, None, true)]
#[case::no_color_takes_precedence_over_missing_netsuke(None, Some("1"), None, false)]
#[case::both_env_vars_disable(None, Some("1"), Some("1"), false)]
fn resolve_output_prefs(
    #[case] no_emoji: Option<bool>,
    #[case] no_color: Option<&str>,
    #[case] no_emoji_env: Option<&str>,
    #[case] expected_emoji: bool,
) {
    let env = fake_env(no_color, no_emoji_env);
    assert_eq!(resolve_with(no_emoji, env).emoji_allowed(), expected_emoji);
}

#[test]
fn emoji_allowed_returns_true_when_permitted() {
    let prefs = resolve_with(Some(false), |_| None);
    assert!(prefs.emoji_allowed());
}

#[test]
fn emoji_allowed_returns_false_when_suppressed() {
    let prefs = resolve_with(Some(true), |_| None);
    assert!(!prefs.emoji_allowed());
}

#[rstest]
#[case::unicode_theme_overrides_no_emoji_env(ThemeResolutionCase {
    theme: Some(ThemePreference::Unicode),
    no_emoji: None,
    mode: OutputMode::Standard,
    no_color: None,
    no_emoji_env: Some("1"),
    expected_emoji: true,
})]
#[case::ascii_theme_stays_ascii_without_env(ThemeResolutionCase {
    theme: Some(ThemePreference::Ascii),
    no_emoji: None,
    mode: OutputMode::Standard,
    no_color: None,
    no_emoji_env: None,
    expected_emoji: false,
})]
#[case::auto_theme_no_color_forces_ascii(ThemeResolutionCase {
    theme: Some(ThemePreference::Auto),
    no_emoji: None,
    mode: OutputMode::Standard,
    no_color: Some("1"),
    no_emoji_env: None,
    expected_emoji: false,
})]
#[case::auto_theme_standard_without_env_uses_unicode(ThemeResolutionCase {
    theme: Some(ThemePreference::Auto),
    no_emoji: None,
    mode: OutputMode::Standard,
    no_color: None,
    no_emoji_env: None,
    expected_emoji: true,
})]
#[case::auto_theme_legacy_no_emoji_stays_ascii(ThemeResolutionCase {
    theme: Some(ThemePreference::Auto),
    no_emoji: Some(true),
    mode: OutputMode::Standard,
    no_color: None,
    no_emoji_env: None,
    expected_emoji: false,
})]
fn resolve_from_theme_with_uses_theme_resolution(#[case] case: ThemeResolutionCase<'_>) {
    let env = fake_env(case.no_color, case.no_emoji_env);
    let prefs = resolve_from_theme_with(case.theme, case.no_emoji, case.mode, env);
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
    #[case] theme: Option<ThemePreference>,
    #[case] prefix_fn: fn(OutputPrefs) -> LocalizedMessage,
    #[case] expected: &str,
) {
    let prefs = resolve_from_theme_with(theme, None, OutputMode::Standard, |_| None);
    assert_eq!(strip_isolates(&prefix_fn(prefs).to_string()), expected);
}

#[rstest]
#[case::accessible_auto(OutputMode::Accessible)]
#[case::standard_ascii(OutputMode::Standard)]
fn spacing_accessors_follow_resolved_theme(#[case] mode: OutputMode) {
    let prefs = resolve_from_theme_with(Some(ThemePreference::Auto), Some(true), mode, |_| None);
    assert_eq!(prefs.task_indent(), "  ");
    assert_eq!(prefs.timing_indent(), "  ");
}
