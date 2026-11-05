//! Unit tests for output mode resolution from environment signals.

use super::*;
use crate::theme::{ThemeContext, ThemePreference, resolve_theme};
use proptest::prelude::*;
use rstest::rstest;

/// Build an environment lookup from optional `NO_COLOR` and `TERM` values.
fn fake_env<'a>(
    no_color: Option<&'a str>,
    term: Option<&'a str>,
) -> impl Fn(&str) -> Option<String> + 'a {
    move |key| match key {
        "NO_COLOR" => no_color.map(String::from),
        "TERM" => term.map(String::from),
        _ => None,
    }
}

#[rstest]
#[case::absent(None, None, false, false)]
#[case::empty_no_color(Some(""), None, false, false)]
#[case::empty_term(None, Some(""), false, false)]
#[case::no_color_zero(Some("0"), Some("xterm-256color"), true, false)]
#[case::term_dumb(None, Some("dumb"), false, true)]
#[case::term_uppercase_dumb(None, Some("DUMB"), false, false)]
#[case::term_leading_space(None, Some(" dumb"), false, false)]
#[case::term_zero(None, Some("0"), false, false)]
#[case::term_xterm(None, Some("xterm-256color"), false, false)]
fn environment_signal_witnesses(
    #[case] no_color: Option<&str>,
    #[case] term: Option<&str>,
    #[case] expected_no_color_set: bool,
    #[case] expected_term_dumb: bool,
) {
    let signals = EnvSignals::read(fake_env(no_color, term));

    assert_eq!(signals.no_color_set, expected_no_color_set);
    assert_eq!(signals.term_dumb, expected_term_dumb);
    assert_eq!(signals.redraw_capable, expected_redraw_capable(term));
}

/// Compare platform-specific redraw capability for the signal witness.
///
/// # Examples
///
/// ```text
/// expected_redraw_capable(Some("xterm-256color")) -> true
/// ```
fn expected_redraw_capable(term: Option<&str>) -> bool {
    #[cfg(unix)]
    {
        term.is_some() && term != Some("dumb")
    }
    #[cfg(not(unix))]
    {
        term != Some("dumb")
    }
}

/// Reference the policy that treats only non-empty `NO_COLOR` as active.
///
/// # Examples
///
/// ```text
/// previous_no_color_active(None, &env_with_no_color) -> true
/// ```
fn expected_no_color_active<F>(colour_policy: Option<ColourPolicy>, read_env: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    match colour_policy {
        Some(ColourPolicy::Always) => false,
        Some(ColourPolicy::Never) => true,
        Some(ColourPolicy::Auto) | None => {
            read_env("NO_COLOR").is_some_and(|value| !value.is_empty())
        }
    }
}

/// Reference output-mode precedence for the environment property.
///
/// # Examples
///
/// ```text
/// previous_resolve_with(None, None, &env_with_no_color) -> Accessible
/// ```
fn expected_resolve_with<F>(
    explicit: Option<bool>,
    colour_policy: Option<ColourPolicy>,
    read_env: &F,
) -> OutputMode
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(forced) = explicit {
        return if forced {
            OutputMode::Accessible
        } else {
            OutputMode::Standard
        };
    }

    if expected_no_color_active(colour_policy, read_env) {
        return OutputMode::Accessible;
    }

    if read_env("TERM").as_deref() == Some("dumb") {
        return OutputMode::Accessible;
    }

    OutputMode::Standard
}

/// Reference the theme resolver's Unicode choice for comparison.
///
/// # Examples
///
/// ```text
/// previous_theme_uses_unicode(None, context, &empty_env) -> true
/// ```
fn expected_theme_uses_unicode<F>(
    theme: Option<ThemePreference>,
    context: ThemeContext,
    read_env: &F,
) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    let no_color = expected_no_color_active(context.colour_policy, read_env);
    match theme {
        Some(ThemePreference::Unicode) => true,
        Some(ThemePreference::Ascii) => false,
        Some(ThemePreference::Auto) | None => {
            if let Some(true) = context.no_emoji {
                return false;
            }
            if no_color {
                return false;
            }
            !context.mode.is_accessible()
        }
    }
}

proptest! {
    #[test]
    fn environment_signal_resolver_matches_reference_policy(
        no_color in prop::option::of(any::<String>()),
        term in prop::option::of(any::<String>()),
        explicit in prop::option::of(any::<bool>()),
        colour_choice in prop::option::of(0_u8..3),
        theme_choice in prop::option::of(0_u8..3),
        no_emoji in prop::option::of(any::<bool>()),
        accessible in any::<bool>(),
    ) {
        let colour_policy = colour_choice.map(|choice| match choice {
            0 => ColourPolicy::Auto,
            1 => ColourPolicy::Always,
            _ => ColourPolicy::Never,
        });
        let theme = theme_choice.map(|choice| match choice {
            0 => ThemePreference::Auto,
            1 => ThemePreference::Unicode,
            _ => ThemePreference::Ascii,
        });
        let mode = if accessible {
            OutputMode::Accessible
        } else {
            OutputMode::Standard
        };
        let read_env = |key: &str| match key {
            "NO_COLOR" => no_color.clone(),
            "TERM" => term.clone(),
            _ => None,
        };
        let context = ThemeContext::new(no_emoji, colour_policy, mode);
        let expected_mode = expected_resolve_with(explicit, colour_policy, &read_env);
        let resolved_mode = resolve_with(explicit, colour_policy, read_env);

        prop_assert_eq!(resolved_mode, expected_mode);

        let expected_emoji_allowed = expected_theme_uses_unicode(theme, context, &read_env);
        let resolved_emoji_allowed = resolve_theme(theme, context, read_env).tokens.emoji_allowed;
        prop_assert_eq!(resolved_emoji_allowed, expected_emoji_allowed);
    }
}

#[rstest]
#[case::explicit_true_forces_accessible(
    Some(true),
    None,
    (None, None),
    OutputMode::Accessible
)]
#[case::explicit_false_overrides_all(
    Some(false),
    None,
    (Some("1"), Some("dumb")),
    OutputMode::Standard
)]
#[case::no_color_triggers_accessible(
    None,
    None,
    (Some("1"), None),
    OutputMode::Accessible
)]
#[case::term_dumb_triggers_accessible(
    None,
    None,
    (None, Some("dumb")),
    OutputMode::Accessible
)]
#[case::normal_term_stays_standard(
    None,
    None,
    (None, Some("xterm-256color")),
    OutputMode::Standard
)]
#[case::no_env_defaults_to_standard(
    None,
    None,
    (None, None),
    OutputMode::Standard
)]
#[case::empty_no_color_is_unset(
    None,
    None,
    (Some(""), None),
    OutputMode::Standard
)]
#[case::no_color_takes_precedence_over_term(
    None,
    None,
    (Some("1"), Some("xterm-256color")),
    OutputMode::Accessible
)]
#[case::explicit_false_overrides_no_color(
    Some(false),
    None,
    (Some("1"), None),
    OutputMode::Standard
)]
#[case::explicit_true_without_env(
    Some(true),
    None,
    (None, Some("xterm-256color")),
    OutputMode::Accessible
)]
#[case::colour_policy_always_ignores_no_color(
    None,
    Some(ColourPolicy::Always),
    (Some("1"), Some("xterm-256color")),
    OutputMode::Standard
)]
#[case::colour_policy_never_forces_accessible(
    None,
    Some(ColourPolicy::Never),
    (None, Some("xterm-256color")),
    OutputMode::Accessible
)]
fn resolve_output_mode(
    #[case] explicit: Option<bool>,
    #[case] colour_policy: Option<ColourPolicy>,
    #[case] env_values: (Option<&str>, Option<&str>),
    #[case] expected: OutputMode,
) {
    let (no_color, term) = env_values;
    let env = fake_env(no_color, term);
    assert_eq!(resolve_with(explicit, colour_policy, env), expected);
}

#[test]
fn is_accessible_returns_true_for_accessible() {
    assert!(OutputMode::Accessible.is_accessible());
}

#[test]
fn is_accessible_returns_false_for_standard() {
    assert!(!OutputMode::Standard.is_accessible());
}
