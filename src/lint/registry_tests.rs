//! Invariants the rule registry must hold for identifiers to stay usable.

use std::collections::BTreeSet;

use super::super::rule::Category;
use super::{all_meta, catalogue, is_known, meta_by_name};

#[test]
fn rule_names_are_unique() {
    let names: Vec<&str> = all_meta().map(|meta| meta.name).collect();
    let unique: BTreeSet<&str> = names.iter().copied().collect();
    assert_eq!(
        names.len(),
        unique.len(),
        "two rules share a name, which would make configuration and suppression ambiguous"
    );
}

/// Names appear in configuration files, suppression comments, documentation
/// anchors, and machine output, so the spelling is constrained to what all four
/// can carry unquoted.
#[test]
fn rule_names_are_lowercase_kebab_case() {
    for meta in all_meta() {
        let is_kebab = !meta.name.is_empty()
            && !meta.name.starts_with('-')
            && !meta.name.ends_with('-')
            && meta
                .name
                .chars()
                .all(|character| character.is_ascii_lowercase() || character == '-');
        assert!(is_kebab, "`{}` is not lowercase kebab-case", meta.name);
    }
}

#[test]
fn every_rule_states_its_summary_rationale_and_remediation() {
    for meta in all_meta() {
        assert!(!meta.summary.is_empty(), "`{}` has no summary", meta.name);
        assert!(
            !meta.rationale.is_empty(),
            "`{}` has no rationale",
            meta.name
        );
        assert!(
            meta.remediation.ends_with('.'),
            "`{}` remediation should be a full instruction",
            meta.name
        );
    }
}

#[test]
fn diagnostic_codes_are_derived_from_the_name() {
    for meta in all_meta() {
        assert_eq!(
            meta.code(),
            format!("netsuke::lint::{}", meta.name.replace('-', "_")),
            "`{}` has an unexpected diagnostic code",
            meta.name
        );
    }
}

#[test]
fn the_catalogue_is_ordered_by_category_then_name() {
    let entries = catalogue();
    let keys: Vec<(&str, &str)> = entries
        .iter()
        .map(|meta| (meta.category.as_str(), meta.name))
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable_by_key(|(category, name)| {
        (Category::parse(category).map(|found| found as usize), *name)
    });
    assert_eq!(
        keys.len(),
        sorted.len(),
        "the catalogue should list every rule once"
    );
    assert!(
        keys.windows(2).all(|pair| match pair {
            [first, second] => first.0 != second.0 || first.1 <= second.1,
            _ => true,
        }),
        "rules within a category should be ordered by name"
    );
}

#[test]
fn lookup_finds_registered_rules_and_rejects_others() {
    let first = catalogue().first().copied().expect("rules should exist");
    assert_eq!(
        meta_by_name(first.name).map(|meta| meta.name),
        Some(first.name)
    );
    assert!(is_known(first.name));
    assert!(meta_by_name("no-such-rule").is_none());
    assert!(!is_known("no-such-rule"));
}
