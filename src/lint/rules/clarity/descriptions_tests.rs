//! Tests for the description clarity rules.

use crate::lint::test_support::{assert_fires, assert_silent, count_for, lint};

/// A manifest with one described and one undescribed action and rule.
const MIXED: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "rules:\n",
    "  - name: described\n",
    "    description: Doing the work\n",
    "    command: \"work {{ outs }}\"\n",
    "  - name: bare\n",
    "    command: \"other {{ outs }}\"\n",
    "actions:\n",
    "  - name: told\n",
    "    description: Run the checks\n",
    "    command: \"check\"\n",
    "  - name: silent\n",
    "    command: \"other\"\n",
    "targets:\n",
    "  - name: a\n",
    "    rule: described\n",
    "  - name: b\n",
    "    rule: bare\n",
);

#[test]
fn action_without_description_reports_only_undescribed_actions() {
    assert_fires(MIXED, "action-without-description", 1);
}

#[test]
fn action_without_description_accepts_a_described_action() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "actions:\n",
        "  - name: test\n",
        "    description: Run the tests\n",
        "    command: \"run tests\"\n",
        "targets: []\n",
    );
    assert_silent(yaml, "action-without-description");
}

#[test]
fn action_without_description_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "actions:\n",
        "  # netsuke-lint: allow action-without-description -- internal helper\n",
        "  - name: helper\n",
        "    command: \"run helper\"\n",
        "targets: []\n",
    );
    assert_silent(yaml, "action-without-description");
}

/// The rule encodes a house style, so it must stay silent until selected.
#[test]
fn rule_without_description_is_off_by_default() {
    let reported: Vec<&str> = lint(MIXED)
        .findings
        .iter()
        .map(|finding| finding.meta.name)
        .collect();
    assert!(
        !reported.contains(&"rule-without-description"),
        "an opt-in rule should not run by default, got {reported:?}"
    );
}

#[test]
fn rule_without_description_reports_when_selected() {
    assert_eq!(count_for(MIXED, "rule-without-description"), 1);
}

#[test]
fn rule_without_description_is_suppressed_by_a_directive() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "rules:\n",
        "  # netsuke-lint: allow rule-without-description -- the command explains itself\n",
        "  - name: bare\n",
        "    command: \"work {{ outs }}\"\n",
        "targets:\n",
        "  - name: a\n",
        "    rule: bare\n",
    );
    assert_silent(yaml, "rule-without-description");
}
