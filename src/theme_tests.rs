//! Tests for theme resolution and token selection.

use super::*;
use rstest::rstest;

fn fake_env<'a>(
    no_color: Option<&'a str>,
    netsuke_no_emoji: Option<&'a str>,
) -> impl Fn(&str) -> Option<String> + 'a {
    move |key| match key {
        "NO_COLOR" => no_color.map(String::from),
        "NETSUKE_NO_EMOJI" => netsuke_no_emoji.map(String::from),
        _ => None,
    }
}

#[rstest]
#[case::explicit_unicode_overrides_all(
    Some(ThemePreference::Unicode),
    Some(true),
    Some("1"),
    Some("1"),
    OutputMode::Accessible,
    true
)]
#[case::explicit_ascii_overrides_all(
    Some(ThemePreference::Ascii),
    Some(false),
    None,
    None,
    OutputMode::Standard,
    false
)]
#[case::auto_defers_to_no_emoji_true(
    Some(ThemePreference::Auto),
    Some(true),
    None,
    None,
    OutputMode::Standard,
    false
)]
#[case::auto_defers_to_netsuke_no_emoji_env(
    Some(ThemePreference::Auto),
    None,
    Some("1"),
    None,
    OutputMode::Standard,
    false
)]
#[case::auto_defers_to_no_color_env(
    Some(ThemePreference::Auto),
    None,
    None,
    Some("1"),
    OutputMode::Standard,
    false
)]
#[case::none_defers_to_no_emoji_true(None, Some(true), None, None, OutputMode::Standard, false)]
#[case::none_defers_to_netsuke_no_emoji_env(
    None,
    None,
    Some("1"),
    None,
    OutputMode::Standard,
    false
)]
#[case::none_defers_to_no_color_env(None, None, None, Some("1"), OutputMode::Standard, false)]
#[case::accessible_mode_uses_ascii(None, None, None, None, OutputMode::Accessible, false)]
#[case::standard_mode_uses_unicode(None, None, None, None, OutputMode::Standard, true)]
#[case::no_emoji_false_defers_to_mode(None, Some(false), None, None, OutputMode::Standard, true)]
fn theme_resolution_precedence(
    #[case] theme: Option<ThemePreference>,
    #[case] no_emoji: Option<bool>,
    #[case] env_no_emoji: Option<&str>,
    #[case] env_no_color: Option<&str>,
    #[case] mode: OutputMode,
    #[case] expect_unicode: bool,
) {
    let resolved = resolve_theme(theme, no_emoji, mode, fake_env(env_no_color, env_no_emoji));
    assert_eq!(resolved.tokens.emoji_allowed, expect_unicode);
    if expect_unicode {
        assert_eq!(resolved.tokens.symbols.success, UNICODE_SYMBOLS.success);
        assert_eq!(resolved.tokens.symbols.error, UNICODE_SYMBOLS.error);
    } else {
        assert_eq!(resolved.tokens.symbols.success, ASCII_SYMBOLS.success);
        assert_eq!(resolved.tokens.symbols.error, ASCII_SYMBOLS.error);
    }
}

#[test]
fn spacing_tokens_are_identical_across_themes() {
    let unicode_theme = resolve_theme(
        Some(ThemePreference::Unicode),
        None,
        OutputMode::Standard,
        |_| None,
    );
    let ascii_theme = resolve_theme(
        Some(ThemePreference::Ascii),
        None,
        OutputMode::Standard,
        |_| None,
    );
    assert_eq!(
        unicode_theme.tokens.spacing, ascii_theme.tokens.spacing,
        "spacing must be identical between Unicode and ASCII themes"
    );
}

#[test]
fn unicode_symbols_contain_non_ascii() {
    assert!(!UNICODE_SYMBOLS.success.is_ascii());
    assert!(!UNICODE_SYMBOLS.error.is_ascii());
}

#[test]
fn ascii_symbols_are_ascii_only() {
    assert!(ASCII_SYMBOLS.success.is_ascii());
    assert!(ASCII_SYMBOLS.error.is_ascii());
    assert!(ASCII_SYMBOLS.warning.is_ascii());
    assert!(ASCII_SYMBOLS.info.is_ascii());
    assert!(ASCII_SYMBOLS.timing.is_ascii());
}
