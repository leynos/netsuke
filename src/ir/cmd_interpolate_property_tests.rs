//! Property tests for command interpolation token boundaries.
//!
//! These properties ensure `interpolate_command` replaces placeholders outside
//! backticks with quoted paths, preserves `$in`, `$out`, and long placeholder
//! tokens verbatim inside backtick-delimited regions, and rejects unbalanced
//! backtick input as an invalid command.

use proptest::prelude::*;
use test_support::ninja_gen::paths_strategy;

use super::{INS_TOKEN, IrGenError, OUTS_TOKEN, interpolate_command};

fn safe_text_strategy() -> impl Strategy<Value = String> {
    // Empty fragments are intentional: surrounding command text may be absent,
    // and trimming whitespace-only generated text exercises that boundary.
    "[a-zA-Z0-9_./ -]{0,24}".prop_map(|text| text.trim().to_owned())
}

proptest! {
    #[test]
    fn dollar_tokens_inside_backticks_are_preserved(prefix in safe_text_strategy(), suffix in safe_text_strategy(), inputs in paths_strategy("in", 1..10), outputs in paths_strategy("out", 1..10)) {
        let template = format!("echo {prefix} `printf '$in $out'` {suffix}");
        let command = interpolate_command(&template, &inputs, &outputs).expect("balanced command should interpolate");

        prop_assert!(command.contains("`printf '$in $out'`"));
    }

    #[test]
    fn long_placeholders_outside_backticks_are_replaced(inputs in paths_strategy("in", 1..10), outputs in paths_strategy("out", 1..10)) {
        let command = interpolate_command(
            &format!("echo {INS_TOKEN} then {OUTS_TOKEN}"),
            &inputs,
            &outputs,
        ).expect("command should interpolate");

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
    fn tokens_inside_backticks_are_preserved_verbatim(token in prop::sample::select(vec!["$in", "$out", INS_TOKEN, OUTS_TOKEN]), inputs in paths_strategy("in", 1..10), outputs in paths_strategy("out", 1..10)) {
        let template = format!("echo `{token}`");
        let command = interpolate_command(&template, &inputs, &outputs).expect("balanced command should interpolate");

        prop_assert_eq!(command, template);
    }

    #[test]
    fn unbalanced_backticks_are_rejected(prefix in safe_text_strategy(), suffix in safe_text_strategy(), inputs in paths_strategy("in", 1..10), outputs in paths_strategy("out", 1..10)) {
        let template = format!("echo {prefix} ` $in {suffix}");
        let err = interpolate_command(&template, &inputs, &outputs).expect_err("unbalanced backticks should fail");

        let is_invalid_command = matches!(err, IrGenError::InvalidCommand { .. });
        prop_assert!(is_invalid_command);
    }
}
