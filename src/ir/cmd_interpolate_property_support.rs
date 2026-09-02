//! Shared strategies and oracles for command-interpolation property tests.
//!
//! This support stays private to `cmd_interpolate_property_tests`: it owns
//! generated templates and independent POSIX scanner assertions, but is not a
//! production interpolation API or a Kani harness dependency.

use proptest::{prelude::*, test_runner::TestCaseError};

use super::super::{
    CommandBindings, INS_TOKEN, IrGenError, OUTS_TOKEN, PathSubstitutions, RecipeShell, substitute,
};

/// Bound every generated template to the documented residual-range maximum.
const MAX_TEMPLATE_LENGTH: usize = 256;

/// Bound dense separators so eight marker placeholders fit within the maximum.
const MAX_DENSE_FRAGMENT_LENGTH: usize = 3;

/// Generate short text that is safe around POSIX command substitutions.
pub(super) fn safe_text_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_./ -]{0,24}".prop_map(|text| text.trim().to_owned())
}

/// Generate adversarial text that may contain interpolation delimiters.
pub(super) fn adversarial_template_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(template_character_strategy(), 0..=64)
        .prop_map(|chars| chars.into_iter().collect())
}

/// Generate templates with sparse literal regions up to the documented bound.
pub(super) fn interpolation_template_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        dense_interpolation_template_strategy(),
        sparse_interpolation_template_strategy(),
    ]
}

/// Generate dense templates containing exactly eight placeholders.
pub(super) fn eight_placeholder_template_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        (interpolation_fragment_strategy(), placeholder_strategy()),
        8,
    )
    .prop_flat_map(|parts| (Just(parts), interpolation_fragment_strategy()))
    .prop_map(|(parts, suffix)| join_template_parts(parts, &suffix))
}

/// Generate raw path bindings that can perturb scanner and guard behaviour.
pub(super) fn raw_binding_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(vec!['a', '`', '\'', '"', ' ']), 0..=3)
        .prop_map(|chars| chars.into_iter().collect())
}

/// Build deliberately unquoted POSIX bindings for scanner and guard properties.
///
/// These inputs bypass path quoting so the properties can isolate placeholder
/// recognition and command validation from the production binding preparer.
pub(super) fn posix_bindings(ins: String, outs: String) -> CommandBindings {
    CommandBindings {
        shell: RecipeShell::Posix,
        ins: raw_path_substitutions(ins),
        outs: raw_path_substitutions(outs),
    }
}

/// Repeat raw path text for every quote context in scanner-only bindings.
fn raw_path_substitutions(path: String) -> PathSubstitutions {
    PathSubstitutions {
        single_quoted: path.clone(),
        double_quoted: path.clone(),
        unquoted: path,
    }
}

/// Assert that the production POSIX scanner agrees with its independent oracle.
pub(super) fn assert_matches_specification(
    template: &str,
    bindings: &CommandBindings,
) -> Result<(), TestCaseError> {
    let specification = specification(template, bindings);
    let substitution = substitute(template, bindings);

    match (substitution, specification) {
        (Ok(actual_output), Ok(expected_output)) => prop_assert_eq!(actual_output, expected_output),
        (Err(IrGenError::InvalidCommand { command, .. }), Err(expected_template)) => {
            prop_assert_eq!(command, expected_template);
        }
        (unexpected_substitution, unexpected_specification) => prop_assert!(
            false,
            "scanner and independent specification disagree: {unexpected_substitution:?} != {unexpected_specification:?}"
        ),
    }
    Ok(())
}

/// Evaluate the independent POSIX scanner specification for prepared bindings.
pub(super) fn specification(template: &str, bindings: &CommandBindings) -> Result<String, String> {
    spec_substitute(template, &bindings.ins.unquoted, &bindings.outs.unquoted)
}

/// Model POSIX placeholder replacement independently of production traversal.
fn spec_substitute(template: &str, ins: &str, outs: &str) -> Result<String, String> {
    let chars = template.chars().collect::<Vec<_>>();
    let mut result = String::new();
    let mut pos = 0;

    while pos < chars.len() {
        let Some(&ch) = chars.get(pos) else {
            break;
        };
        let substitution = spec_match(&chars, pos, ins, outs);
        if is_protected_by_backticks(&chars, pos) && substitution.is_some() {
            return Err(template.to_owned());
        }
        if let Some((replacement, width)) = substitution {
            result.push_str(replacement);
            pos += width;
        } else {
            result.push(ch);
            pos += 1;
        }
    }
    Ok(result)
}

/// Report whether `pos` falls inside a POSIX backtick-protected region.
fn is_protected_by_backticks(chars: &[char], pos: usize) -> bool {
    chars.iter().take(pos).filter(|&&ch| ch == '`').count() & 1 == 1
}

/// Match one supported placeholder at `pos` in the independent specification.
fn spec_match<'a>(
    chars: &[char],
    pos: usize,
    ins: &'a str,
    outs: &'a str,
) -> Option<(&'a str, usize)> {
    if token_matches(chars, pos, INS_TOKEN) {
        return Some((ins, INS_TOKEN.chars().count()));
    }
    token_matches(chars, pos, OUTS_TOKEN).then_some((outs, OUTS_TOKEN.chars().count()))
}

/// Report whether `token` begins at `pos` in `chars`.
fn token_matches(chars: &[char], pos: usize, token: &str) -> bool {
    token
        .chars()
        .enumerate()
        .all(|(offset, ch)| chars.get(pos + offset) == Some(&ch))
}

/// Report whether `command` contains an unmatched POSIX backtick.
pub(super) fn has_odd_backticks(command: &str) -> bool {
    command.chars().filter(|&ch| ch == '`').count() & 1 == 1
}

/// Generate one character that can interact with interpolation syntax.
fn template_character_strategy() -> impl Strategy<Value = char> {
    prop::sample::select(vec!['$', '`', 'i', 'n', 'o', 'u', 't', '_', 'a', ' '])
}

/// Generate a short fragment for dense eight-placeholder templates.
fn interpolation_fragment_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(template_character_strategy(), 0..=MAX_DENSE_FRAGMENT_LENGTH)
        .prop_map(|chars| chars.into_iter().collect())
}

/// Generate one manifest-owned input or output placeholder.
fn placeholder_strategy() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![INS_TOKEN, OUTS_TOKEN])
}

/// Generate dense templates while retaining the 256-character upper bound.
fn dense_interpolation_template_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        (interpolation_fragment_strategy(), placeholder_strategy()),
        0..=8,
    )
    .prop_flat_map(|parts| (Just(parts), interpolation_fragment_strategy()))
    .prop_map(|(parts, suffix)| join_template_parts(parts, &suffix))
}

/// Generate sparse templates with long literal prefixes, suffixes, or both.
fn sparse_interpolation_template_strategy() -> impl Strategy<Value = String> {
    let maximum_literal_length = MAX_TEMPLATE_LENGTH - OUTS_TOKEN.len();
    (0..=maximum_literal_length)
        .prop_flat_map(move |prefix_length| {
            (
                prop::collection::vec(template_character_strategy(), prefix_length),
                placeholder_strategy(),
                prop::collection::vec(
                    template_character_strategy(),
                    0..=(maximum_literal_length - prefix_length),
                ),
            )
        })
        .prop_map(|(prefix, placeholder, suffix)| {
            let mut template = prefix.into_iter().collect::<String>();
            template.push_str(placeholder);
            template.extend(suffix);
            template
        })
}

/// Join generated fragments and placeholders into one template.
fn join_template_parts(parts: Vec<(String, &'static str)>, suffix: &str) -> String {
    let mut template = String::new();
    for (fragment, placeholder) in parts {
        template.push_str(&fragment);
        template.push_str(placeholder);
    }
    template.push_str(suffix);
    template
}
