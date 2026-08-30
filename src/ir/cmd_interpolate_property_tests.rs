//! Property tests for command interpolation token boundaries.
//!
//! These properties ensure `interpolate_command` replaces manifest markers outside
//! protected regions with quoted paths, rejects markers within them, and
//! rejects unbalanced backtick input as an invalid command.

use proptest::prelude::*;
use test_support::ninja_gen::paths_strategy;

use super::{
    CommandBindings, INS_TOKEN, IrGenError, OUTS_TOKEN, RecipeShell,
    interpolate_command_with_bindings, interpolate_command_with_shell, substitute,
};


//! Property tests for command interpolation token boundaries.
//!
//! These properties ensure `interpolate_command` replaces manifest markers outside
//! protected regions with quoted paths, rejects markers within them, and
//! rejects unbalanced backtick input as an invalid command.
};

fn safe_text_strategy() -> impl Strategy<Value = String> {
    // Empty fragments are intentional: surrounding command text may be absent,
    // and trimming whitespace-only generated text exercises that boundary.
    "[a-zA-Z0-9_./ -]{0,24}".prop_map(|text| text.trim().to_owned())
}

fn adversarial_template_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec![
            '$', '`', 'i', 'n', 'o', 'u', 't', '_', 'a', '\'', '\\', ' ',
        ]),
        0..=64,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

fn interpolation_fragment_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec![
            '$', '`', 'i', 'n', 'o', 'u', 't', '_', 'a', '\'', '"', '\\', ' ',
        ]),
        0..=4,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

fn placeholder_strategy() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec!["$in", "$out", INS_TOKEN, OUTS_TOKEN])
}

fn interpolation_template_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        (interpolation_fragment_strategy(), placeholder_strategy()),
        0..=8,
    )
    .prop_flat_map(|parts| (Just(parts), interpolation_fragment_strategy()))
    .prop_map(|(parts, suffix)| join_template_parts(parts, &suffix))
}

fn eight_placeholder_template_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        (interpolation_fragment_strategy(), placeholder_strategy()),
        8,
    )
    .prop_flat_map(|parts| (Just(parts), interpolation_fragment_strategy()))
    .prop_map(|(parts, suffix)| join_template_parts(parts, &suffix))
}

fn join_template_parts(parts: Vec<(String, &'static str)>, suffix: &str) -> String {
    let mut template = String::new();
    for (fragment, placeholder) in parts {
        template.push_str(&fragment);
        template.push_str(placeholder);
    }
    template.push_str(suffix);
    template
}

fn raw_binding_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(vec!['a', '`', '\'', '"', ' ']), 0..=3)
        .prop_map(|chars| chars.into_iter().collect())
}

/// Build deliberately unquoted POSIX bindings for scanner and guard properties.
///
/// These inputs bypass path quoting so the properties can isolate placeholder
/// recognition and command validation from the production binding preparer.
fn posix_bindings(ins: String, outs: String) -> CommandBindings {
    CommandBindings {
        shell: RecipeShell::Posix,
        ins,
        outs,
    }
}

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

fn is_protected_by_backticks(chars: &[char], pos: usize) -> bool {
    chars.iter().take(pos).filter(|&&ch| ch == '`').count() & 1 == 1
}

fn spec_match<'a>(
    chars: &[char],
    pos: usize,
    ins: &'a str,
    outs: &'a str,
) -> Option<(&'a str, usize)> {
    if chars.get(pos) == Some(&'$') {
        if token_matches(chars, pos + 1, "in") && sigil_boundaries_are_valid(chars, pos, 2) {
            return Some((ins, 3));
        }
        if token_matches(chars, pos + 1, "out") && sigil_boundaries_are_valid(chars, pos, 3) {
            return Some((outs, 4));
        }
    }
    if token_matches(chars, pos, INS_TOKEN) {
        return Some((ins, INS_TOKEN.chars().count()));
    }
    token_matches(chars, pos, OUTS_TOKEN).then_some((outs, OUTS_TOKEN.chars().count()))
}

fn sigil_boundaries_are_valid(chars: &[char], pos: usize, pattern_len: usize) -> bool {
    chars
        .get(pos.wrapping_sub(1))
        .is_none_or(|ch| !is_spec_identifier(*ch))
        && chars
            .get(pos + pattern_len + 1)
            .is_none_or(|ch| !is_spec_identifier(*ch))
}

fn token_matches(chars: &[char], pos: usize, token: &str) -> bool {
    token
        .chars()
        .enumerate()
        .all(|(offset, ch)| chars.get(pos + offset) == Some(&ch))
}

const fn is_spec_identifier(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn has_odd_backticks(command: &str) -> bool {
    command.chars().filter(|&ch| ch == '`').count() & 1 == 1
}

proptest! {
    /// Reject manifest markers in backticks for every generated path binding.
    #[test]
    fn manifest_tokens_inside_backticks_are_rejected(prefix in safe_text_strategy(), suffix in safe_text_strategy(), inputs in paths_strategy("in", 1..10), outputs in paths_strategy("out", 1..10)) {
        let template = format!("echo {prefix} `printf '{INS_TOKEN} {OUTS_TOKEN}'` {suffix}");
        let error = interpolate_command_with_shell(&template, &inputs, &outputs, RecipeShell::Posix)
            .expect_err("placeholders inside backticks should be rejected");

        prop_assert!(
            matches!(error, IrGenError::InvalidCommand { .. }),
            "backtick placeholder must be an invalid command: {error:?}"
        );
    }

    #[test]
    fn long_placeholders_outside_backticks_are_replaced(inputs in paths_strategy("in", 1..10), outputs in paths_strategy("out", 1..10)) {
        let command = interpolate_command_with_shell(
            &format!("echo {INS_TOKEN} then {OUTS_TOKEN}"),
            &inputs,
            &outputs,
            RecipeShell::Posix,
        )
        .expect("command should interpolate");

        prop_assert!(!command.contains(INS_TOKEN));
        prop_assert!(!command.contains(OUTS_TOKEN));
        for input in inputs {
            prop_assert!(command.contains(input.as_str()));
        }
        for output in outputs {
            prop_assert!(command.contains(output.as_str()));
        }
    }

    #[test]
    fn dollar_prefixed_shell_variables_are_preserved(inputs in paths_strategy("in", 1..10), outputs in paths_strategy("out", 1..10)) {
        let command = interpolate_command_with_shell(
            "echo $in then $out then $ins then $outs",
            &inputs,
            &outputs,
            RecipeShell::Posix,
        )
        .expect("literal shell variables should remain valid");

        prop_assert_eq!(command, "echo $in then $out then $ins then $outs");
    }

    /// Quote apostrophe-bearing PowerShell paths as single literals.
    #[test]
    fn power_shell_bindings_double_apostrophes_in_paths(
        prefix in "[a-zA-Z0-9]{0,12}",
        suffix in "[a-zA-Z0-9]{0,12}",
    ) {
        let input = camino::Utf8PathBuf::from(format!("{prefix}'input"));
        let output = camino::Utf8PathBuf::from(format!("{suffix}'output"));
        let command = interpolate_command_with_shell(
            &format!("Write-Output {INS_TOKEN} {OUTS_TOKEN}"),
            std::slice::from_ref(&input),
            std::slice::from_ref(&output),
            RecipeShell::PowerShell,
        )
        .expect("PowerShell command should interpolate");

        let expected_input = format!("'{}'", input.as_str().replace('\'', "''"));
        let expected_output = format!("'{}'", output.as_str().replace('\'', "''"));
        prop_assert!(command.contains(&expected_input));
        prop_assert!(command.contains(&expected_output));
    }

    /// Reject every manifest marker when backticks protect it.
    #[test]
    fn tokens_inside_backticks_are_rejected(token in prop::sample::select(vec![INS_TOKEN, OUTS_TOKEN]), inputs in paths_strategy("in", 1..10), outputs in paths_strategy("out", 1..10)) {
        let template = format!("echo `{token}`");
        let error = interpolate_command_with_shell(&template, &inputs, &outputs, RecipeShell::Posix)
            .expect_err("placeholders inside backticks should be rejected");

        prop_assert!(
            matches!(error, IrGenError::InvalidCommand { .. }),
            "backtick placeholder must be an invalid command: {error:?}"
        );
    }

    #[test]
    fn adversarial_text_rejects_protected_tokens(template in adversarial_template_strategy()) {
        let backtick_count = template.chars().filter(|&ch| ch == '`').count();
        let has_open_backtick = backtick_count & 1 == 1;
        let protected = if has_open_backtick { "$in $out`" } else { "`$in $out`" };
        let result = interpolate_command_with_shell(
            &format!("{template}{protected}"),
            &[],
            &[],
            RecipeShell::Posix,
        );
        let is_invalid_command = matches!(result, Err(IrGenError::InvalidCommand { .. }));

        prop_assert!(is_invalid_command);
    }

    #[test]
    fn unbalanced_backticks_are_rejected(prefix in safe_text_strategy(), suffix in safe_text_strategy(), inputs in paths_strategy("in", 1..10), outputs in paths_strategy("out", 1..10)) {
        let template = format!("echo {prefix} ` {INS_TOKEN} {suffix}");
        let err = interpolate_command_with_shell(&template, &inputs, &outputs, RecipeShell::Posix)
            .expect_err("unbalanced backticks should fail");

        let is_invalid_command = matches!(err, IrGenError::InvalidCommand { .. });
        prop_assert!(is_invalid_command);
    }

    #[test]
    fn scanner_agrees_with_independent_specification(
        template in interpolation_template_strategy(),
        ins in raw_binding_strategy(),
        outs in raw_binding_strategy(),
    ) {
        let bindings = posix_bindings(ins, outs);
        let specification = spec_substitute(&template, &bindings.ins, &bindings.outs);
        let substitution = substitute(&template, &bindings);

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
    }

    #[test]
    fn scanner_covers_eight_placeholder_templates(
        template in eight_placeholder_template_strategy(),
        ins in raw_binding_strategy(),
        outs in raw_binding_strategy(),
    ) {
        let bindings = posix_bindings(ins, outs);
        let specification = spec_substitute(&template, &bindings.ins, &bindings.outs);
        let substitution = substitute(&template, &bindings);

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
    }

    #[test]
    fn substituted_odd_backticks_are_rejected(
        template in interpolation_template_strategy(),
        ins in raw_binding_strategy(),
        outs in raw_binding_strategy(),
    ) {
        let bindings = posix_bindings(ins, outs);
        let specification = spec_substitute(&template, &bindings.ins, &bindings.outs);
        let outcome = interpolate_command_with_bindings(&template, &bindings);

        if let Ok(substituted) = specification
            && has_odd_backticks(&substituted)
        {
            match outcome {
                Err(IrGenError::InvalidCommand { command, .. }) => {
                    prop_assert_eq!(command, substituted);
                }
                unexpected_outcome => prop_assert!(
                    false,
                    "odd substituted command was accepted: {unexpected_outcome:?}"
                ),
            }
        }
    }

    #[test]
    fn guard_uses_the_substituted_command(
        template in interpolation_template_strategy(),
        ins in raw_binding_strategy(),
        outs in raw_binding_strategy(),
    ) {
        let bindings = posix_bindings(ins, outs);
        let specification = spec_substitute(&template, &bindings.ins, &bindings.outs);

        match (specification, interpolate_command_with_bindings(&template, &bindings)) {
            (Ok(expected_command), Ok(command)) => {
                let is_valid = !has_odd_backticks(&expected_command)
                    && shlex::split(&expected_command).is_some();
                prop_assert!(is_valid, "guard accepted an invalid substituted command");
                prop_assert_eq!(command, expected_command);
            }
            (Ok(expected_command), Err(IrGenError::InvalidCommand { command, .. })) => {
                let is_valid = !has_odd_backticks(&expected_command)
                    && shlex::split(&expected_command).is_some();
                prop_assert!(!is_valid, "guard rejected a valid substituted command");
                prop_assert_eq!(command, expected_command);
            }
            (Err(expected_template), Err(IrGenError::InvalidCommand { command, .. })) => {
                prop_assert_eq!(command, expected_template);
            }
            (unexpected_specification, unexpected_outcome) => prop_assert!(
                false,
                "guard and independent specification disagree: {unexpected_outcome:?} != {unexpected_specification:?}"
            ),
        }
    }
}
