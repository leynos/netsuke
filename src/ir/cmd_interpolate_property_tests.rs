//! Property tests for command interpolation token boundaries.
//!
//! These properties ensure `interpolate_command` replaces manifest markers outside
//! protected regions with quoted paths, rejects markers within them, and
//! rejects unbalanced backtick input as an invalid command.

use proptest::prelude::*;
use test_support::ninja_gen::paths_strategy;

use super::{INS_TOKEN, IrGenError, OUTS_TOKEN, RecipeShell, interpolate_command_with_shell};

fn safe_text_strategy() -> impl Strategy<Value = String> {
    // Empty fragments are intentional: surrounding command text may be absent,
    // and trimming whitespace-only generated text exercises that boundary.
    "[a-zA-Z0-9_./ -]{0,24}".prop_map(|text| text.trim().to_owned())
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
            "echo $in then $out",
            &inputs,
            &outputs,
            RecipeShell::Posix,
        )
        .expect("literal shell variables should remain valid");

        prop_assert_eq!(command, "echo $in then $out");
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
    fn unbalanced_backticks_are_rejected(prefix in safe_text_strategy(), suffix in safe_text_strategy(), inputs in paths_strategy("in", 1..10), outputs in paths_strategy("out", 1..10)) {
        let template = format!("echo {prefix} ` {INS_TOKEN} {suffix}");
        let err = interpolate_command_with_shell(&template, &inputs, &outputs, RecipeShell::Posix)
            .expect_err("unbalanced backticks should fail");

        let is_invalid_command = matches!(err, IrGenError::InvalidCommand { .. });
        prop_assert!(is_invalid_command);
    }
}
