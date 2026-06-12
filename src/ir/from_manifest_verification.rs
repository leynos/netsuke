//! Kani harnesses for manifest-to-IR safety properties.

use super::*;
use camino::Utf8PathBuf;

#[kani::proof]
#[kani::unwind(12)]
fn duplicate_output_always_rejected() {
    let outputs = vec![Utf8PathBuf::from("out"), Utf8PathBuf::from("out")];
    let targets = IrHashMap::default();

    match find_duplicates(&outputs, &targets) {
        Some(dups) => {
            kani::assert(dups.len() == 1, "one duplicate output is reported");
            kani::assert(dups[0].as_str() == "out", "reported duplicate is preserved");
        }
        _ => kani::assert(false, "expected DuplicateOutput"),
    }
}

#[kani::proof]
#[kani::unwind(6)]
fn empty_rule_shape_is_rejected() {
    let rule_map = IrHashMap::default();

    match resolve_rule(&StringOrList::Empty, &rule_map, "out") {
        Err(IrGenError::EmptyRule { target_name, .. }) => {
            kani::assert(target_name == "out", "empty-rule target is preserved");
        }
        _ => kani::assert(false, "empty rule shape must select EmptyRule"),
    }
}

#[kani::proof]
#[kani::unwind(8)]
fn multiple_rule_shape_is_rejected() {
    let rule_map = IrHashMap::default();
    let rule = StringOrList::List(vec!["a".to_owned(), "b".to_owned()]);

    match resolve_rule(&rule, &rule_map, "out") {
        Err(IrGenError::MultipleRules {
            target_name, rules, ..
        }) => {
            kani::assert(target_name == "out", "multiple-rule target is preserved");
            kani::assert(rules.len() == 2, "both provided rule names are reported");
            kani::assert(rules[0] == "a", "multiple-rule names are preserved");
            kani::assert(rules[1] == "b", "multiple-rule names are preserved");
        }
        _ => kani::assert(false, "list rule shape must select MultipleRules"),
    }
}

#[kani::proof]
#[kani::unwind(6)]
fn missing_rule_shape_is_rejected() {
    let rule_map = IrHashMap::default();
    let rule = StringOrList::String("m".to_owned());

    match resolve_rule(&rule, &rule_map, "out") {
        Err(IrGenError::RuleNotFound {
            target_name,
            rule_name,
            ..
        }) => {
            kani::assert(target_name == "out", "missing-rule target is preserved");
            kani::assert(rule_name == "m", "missing rule name is preserved");
        }
        _ => kani::assert(false, "missing rule shape must select RuleNotFound"),
    }
}
