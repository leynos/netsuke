//! Property tests for Bash ANSI-C markers, checked against an independent model.
//!
//! A marker reached while `$'…'` is open cannot use the single-quoted binding:
//! a backslash there escapes the next character, so a path ending in one can
//! close the string early and leave the rest executable (CWE-78, finding
//! `b247e1c0-c546-4ac4-a8e1-ddb892fc9cee`). Both POSIX traversals reject it.
//!
//! The expectation comes from the `model` child, which states Bash's lexical
//! rules from first principles and never calls the production scanner.

use proptest::prelude::*;

use super::super::{
    CommandBindings, INS_TOKEN, IrGenError, RecipeShell, interpolate_command_with_bindings,
    interpolate_script_with_bindings,
};
use super::support::{context_sentinel_bindings, is_valid_posix_command};

mod model;

use model::AnsiCModel;

/// Name one recipe-lowering function pointer.
type Interpolate = fn(&str, &CommandBindings) -> Result<String, IrGenError>;

/// Name the POSIX lowering entry points and their diagnostic labels.
const LOWERINGS: [(&str, Interpolate); 2] = [
    ("command", interpolate_command_with_bindings),
    ("script", interpolate_script_with_bindings),
];

/// Generate one source fragment the ANSI-C rules must distinguish.
fn fragment_strategy() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        4 => Just("$"),
        4 => Just("'"),
        2 => Just("\\"),
        2 => Just("\n"),
        1 => Just("\\\n"),
        1 => Just("\""),
        2 => Just("$("),
        2 => Just(")"),
        1 => Just("`"),
        1 => Just("#"),
        // No `case` spelling is possible from these words.
        3 => prop::sample::select(vec!["a", "b", "x", "word", "value", "name"]),
    ]
}

/// Offer leading fragments that cannot open a quote or a comment.
const CLEAN_FRAGMENTS: [&str; 6] = ["$", "\\", "\n", "\\\n", ")", "word"];

/// Generate a prefix whose explicit `$'` can open an ANSI-C string.
fn ansi_c_prefix_strategy() -> impl Strategy<Value = String> {
    let leading = prop::collection::vec(prop::sample::select(CLEAN_FRAGMENTS.to_vec()), 0..=4);
    leading.prop_map(|fragments| format!("{}$'", fragments.concat()))
}

/// Generate text holding a complete ANSI-C string.
///
/// The newline before the closing quote stops a trailing backslash escaping it.
fn ansi_c_text_strategy() -> impl Strategy<Value = String> {
    let body = prop::collection::vec(fragment_strategy(), 0..=4);
    body.prop_map(|fragments| format!("echo $'{}\n'", fragments.concat()))
}

/// Select the sentinel bindings for one recipe shell.
fn sentinel_bindings(shell: RecipeShell) -> CommandBindings {
    let bindings = context_sentinel_bindings();
    CommandBindings { shell, ..bindings }
}

/// Insert `marker` at one character boundary of `chars`.
fn template_with_marker(chars: &[char], position: usize, marker: &str) -> String {
    let head: String = chars.iter().take(position).collect();
    let tail: String = chars.iter().skip(position).collect();
    format!("{head}{marker}{tail}")
}

/// Assert that a marker inside ANSI-C is rejected with its original template.
fn assert_rejected(template: &str, bindings: &CommandBindings) -> Result<(), TestCaseError> {
    for (name, interpolate) in LOWERINGS {
        let outcome = interpolate(template, bindings);
        prop_assert!(
            matches!(&outcome, Err(IrGenError::InvalidCommand { command, .. })
                if command.as_str() == template),
            "a marker inside ANSI-C via {name} for {:?} must report {template:?}: {outcome:?}",
            bindings.shell
        );
    }
    Ok(())
}

/// Assert that both traversals return marker-free text unchanged.
fn assert_preserved(
    template: &str,
    expected: &str,
    bindings: &CommandBindings,
) -> Result<(), TestCaseError> {
    for (name, interpolate) in LOWERINGS {
        let outcome = interpolate(template, bindings);
        prop_assert!(
            matches!(&outcome, Ok(output) if output.as_str() == expected),
            "the {name} traversal for {:?} must return {expected:?}: {outcome:?}",
            bindings.shell
        );
    }
    Ok(())
}

proptest! {
    /// Reject every marker the model places inside an open ANSI-C string.
    #[test]
    fn markers_inside_open_ansi_c_are_rejected(prefix in ansi_c_prefix_strategy()) {
        let chars: Vec<char> = prefix.chars().collect();
        let model = AnsiCModel::scan(&chars);
        prop_assume!(model.first_ansi_c.is_some(), "no ANSI-C opener in {:?}", prefix);
        let Some(position) = model.first_ansi_c else {
            return Ok(());
        };
        let template = template_with_marker(&chars, position.index(), INS_TOKEN);
        for shell in [RecipeShell::Posix, RecipeShell::Bash] {
            assert_rejected(&template, &sentinel_bindings(shell))?;
        }
    }

    /// Preserve marker-free ANSI-C text byte-for-byte within the command guard.
    #[test]
    fn marker_free_ansi_c_text_is_preserved(prefix in ansi_c_text_strategy()) {
        let chars: Vec<char> = prefix.chars().collect();
        let model = AnsiCModel::scan(&chars);
        prop_assert!(model.first_ansi_c.is_some(), "the opener is missing in {:?}", prefix);
        prop_assume!(!model.has_escaped_ansi_c, "escaped ANSI-C in {:?}", prefix);
        prop_assume!(is_valid_posix_command(&prefix), "invalid command {:?}", prefix);
        for shell in [RecipeShell::Posix, RecipeShell::Bash] {
            assert_preserved(&prefix, &prefix, &sentinel_bindings(shell))?;
        }
    }

    /// Encode a marker after a closed ANSI-C string as unquoted text.
    ///
    /// The sentinel bindings name their quote context, so this reads the
    /// encoding the traversal selected instead of restating it. Prefixes whose
    /// string body holds a backslash are skipped, because lowering refuses
    /// that shape before any marker is considered.
    #[test]
    fn markers_after_closed_ansi_c_take_the_unquoted_encoding(prefix in ansi_c_text_strategy()) {
        let chars: Vec<char> = prefix.chars().collect();
        let model = AnsiCModel::scan(&chars);
        prop_assume!(!model.has_unmodelled_region, "unmodelled region in {:?}", prefix);
        prop_assume!(!model.has_escaped_ansi_c, "escaped ANSI-C in {:?}", prefix);
        let maybe_position = model.last_unquoted_after_closed;
        prop_assume!(maybe_position.is_some(), "no closed region in {:?}", prefix);
        let Some(position) = maybe_position else {
            return Ok(());
        };
        let template = template_with_marker(&chars, position.index(), INS_TOKEN);
        let expected = template_with_marker(&chars, position.index(), "UNQ");
        prop_assume!(is_valid_posix_command(&expected), "invalid command {:?}", expected);
        for shell in [RecipeShell::Posix, RecipeShell::Bash] {
            assert_preserved(&template, &expected, &sentinel_bindings(shell))?;
        }
    }
}
