//! Kani harnesses for manifest-to-IR safety properties.

use super::*;
use camino::Utf8PathBuf;

use crate::ast::StringOrList;

/// Prove duplicate paths in one target are reported before edge insertion.
#[kani::proof]
#[kani::unwind(12)]
fn duplicate_output_always_rejected() {
    let output_name = symbolic_path_name();
    let outputs = vec![
        Utf8PathBuf::from(output_name.as_str()),
        Utf8PathBuf::from(output_name.as_str()),
    ];
    let targets = IrHashMap::default();

    match find_duplicates(&outputs, &targets) {
        Some(dups) => {
            kani::assert(dups.len() == 1, "one duplicate output is reported");
            kani::assert(
                dups[0].as_str() == output_name.as_str(),
                "reported duplicate output is preserved",
            );
        }
        None => kani::assert(false, "expected duplicate output"),
    }
}

/// Prove an empty rule selector reaches the empty-rule error path.
#[kani::proof]
#[kani::unwind(6)]
fn empty_rule_shape_is_rejected() {
    let rule_map = IrHashMap::default();
    let expected_target_name = symbolic_path_name();

    match resolve_rule(&StringOrList::Empty, &rule_map, &expected_target_name) {
        Err(IrGenError::EmptyRule { target_name, .. }) => {
            kani::assert(
                target_name == expected_target_name,
                "empty-rule target is preserved",
            );
        }
        _ => kani::assert(false, "empty rule shape must select EmptyRule"),
    }
}

/// Prove a multi-rule selector reports every provided rule name.
#[kani::proof]
#[kani::unwind(8)]
fn multiple_rule_shape_is_rejected() {
    let rule_map = IrHashMap::default();
    let expected_target_name = symbolic_path_name();
    let rule = if kani::any::<bool>() {
        StringOrList::List(vec!["a".to_owned(), "b".to_owned()])
    } else {
        StringOrList::List(vec!["b".to_owned(), "a".to_owned()])
    };

    match resolve_rule(&rule, &rule_map, &expected_target_name) {
        Err(IrGenError::MultipleRules {
            target_name, rules, ..
        }) => {
            kani::assert(
                target_name == expected_target_name,
                "multiple-rule target is preserved",
            );
            kani::assert(rules.len() == 2, "both provided rule names are reported");
            kani::assert(rules[0] == "a", "multiple-rule names are preserved");
            kani::assert(rules[1] == "b", "multiple-rule names are preserved");
        }
        _ => kani::assert(false, "list rule shape must select MultipleRules"),
    }
}

/// Prove an unknown single rule selector preserves target and rule names.
#[kani::proof]
#[kani::unwind(6)]
fn missing_rule_shape_is_rejected() {
    let rule_map = IrHashMap::default();
    let expected_target_name = symbolic_path_name();
    let expected_rule_name = symbolic_rule_name();
    let rule = StringOrList::String(expected_rule_name.clone());

    match resolve_rule(&rule, &rule_map, &expected_target_name) {
        Err(IrGenError::RuleNotFound {
            target_name,
            rule_name,
            ..
        }) => {
            kani::assert(
                target_name == expected_target_name,
                "missing-rule target is preserved",
            );
            kani::assert(
                rule_name == expected_rule_name,
                "missing rule name is preserved",
            );
        }
        _ => kani::assert(false, "missing rule shape must select RuleNotFound"),
    }
}

fn symbolic_path_name() -> String {
    if kani::any::<bool>() {
        "a".to_owned()
    } else {
        "b".to_owned()
    }
}

fn symbolic_rule_name() -> String {
    if kani::any::<bool>() {
        "m".to_owned()
    } else {
        "n".to_owned()
    }
}
