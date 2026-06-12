//! Kani harnesses for manifest-to-IR safety properties.

use super::*;

#[kani::proof]
fn duplicate_output_always_rejected() {
    let output_seed = kani::any::<u8>();
    kani::assume(output_seed < 2);

    let expected_output = symbolic_output_name(output_seed);
    let duplicates = vec![Utf8PathBuf::from(expected_output)];

    match duplicate_output_error_from_paths(duplicates) {
        IrGenError::DuplicateOutput { outputs, .. } => {
            kani::assert(outputs.len() == 1, "one duplicate output is reported");
            kani::assert(
                output_matches(outputs.first(), expected_output),
                "reported duplicate must include the shared path",
            );
        }
        _ => kani::assert(false, "expected DuplicateOutput"),
    }
}

#[kani::proof]
fn rule_selection_errors_match_rule_shape() {
    let rule_seed = kani::any::<u8>();
    kani::assume(rule_seed < 3);

    match (rule_seed, symbolic_rule_selection_result(rule_seed)) {
        (0, Err(IrGenError::EmptyRule { target_name, .. })) => {
            kani::assert(target_name == "out", "empty-rule target is preserved");
        }
        (
            1,
            Err(IrGenError::MultipleRules {
                target_name, rules, ..
            }),
        ) => {
            kani::assert(target_name == "out", "multiple-rule target is preserved");
            kani::assert(rules.len() == 2, "both provided rule names are reported");
            kani::assert(rules[0] == "a", "multiple-rule names are preserved");
            kani::assert(rules[1] == "b", "multiple-rule names are preserved");
        }
        (
            2,
            Err(IrGenError::RuleNotFound {
                target_name,
                rule_name,
                ..
            }),
        ) => {
            kani::assert(target_name == "out", "missing-rule target is preserved");
            kani::assert(rule_name == "m", "missing rule name is preserved");
        }
        _ => kani::assert(false, "rule shape must select the matching error"),
    }
}

fn symbolic_output_name(seed: u8) -> &'static str {
    if seed == 0 { "out-a" } else { "out-b" }
}

fn symbolic_rule_selection_result(seed: u8) -> Result<(), IrGenError> {
    let target_name = "out";
    match seed {
        0 => Err(empty_rule_error(target_name)),
        1 => Err(multiple_rules_error(
            target_name,
            vec!["a".to_owned(), "b".to_owned()],
        )),
        _ => Err(rule_not_found_error(target_name, "m")),
    }
}

fn output_matches(actual: Option<&String>, expected: &str) -> bool {
    actual.is_some_and(|output| output == expected)
}
