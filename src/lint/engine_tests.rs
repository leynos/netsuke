//! Engine invariants that are not specific to any one rule.

use proptest::prelude::*;

use crate::lint::finding::Finding;
use crate::lint::policy::Policy;
use crate::lint::severity::Severity;
use crate::lint::test_support::{lint, lint_with};

/// A manifest that trips several rules at more than one severity.
const NOISY: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "vars:\n",
    "  spare: unused\n",
    "actions:\n",
    "  - name: clean\n",
    "    command: \"rm -rf build\"\n",
    "targets:\n",
    "  - name: out.txt\n",
    "    command: \"cp $$SRC out.txt && [[ -f out.txt ]]\"\n",
);

#[test]
fn findings_are_ordered_by_position_then_rule() {
    let findings = lint(NOISY).findings;
    assert!(findings.len() > 1, "the fixture should trip several rules");
    let keys: Vec<_> = findings.iter().map(Finding::sort_key).collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "findings should be emitted in sort order");
}

/// Two runs over the same manifest must agree, including across the hash-map
/// iteration orders inside the build graph.
#[test]
fn two_runs_over_one_manifest_agree() {
    let first: Vec<String> = lint(NOISY)
        .findings
        .iter()
        .map(Finding::display_message)
        .collect();
    for _ in 0..8 {
        let repeat: Vec<String> = lint(NOISY)
            .findings
            .iter()
            .map(Finding::display_message)
            .collect();
        assert_eq!(repeat, first, "linting should be deterministic");
    }
}

/// The engine, not the rule, stamps severity, so every finding must carry the
/// severity policy resolved for its rule.
#[test]
fn every_finding_carries_its_policy_severity() {
    let policy =
        Policy::resolve(&["redundancy=error", "hygiene=advice"]).expect("selectors should resolve");
    for finding in lint_with(NOISY, &policy).findings {
        assert_eq!(
            Some(finding.severity),
            policy.severity_of(finding.meta.name),
            "`{}` reported at the wrong severity",
            finding.meta.name
        );
    }
}

#[test]
fn a_disabled_rule_reports_nothing() {
    let reported: Vec<&str> = lint(NOISY)
        .findings
        .iter()
        .map(|finding| finding.meta.name)
        .collect();
    assert!(
        reported.contains(&"unused-var"),
        "the fixture should trip it"
    );

    let policy = Policy::resolve(&["unused-var=off"]).expect("selector should resolve");
    let after: Vec<&str> = lint_with(NOISY, &policy)
        .findings
        .iter()
        .map(|finding| finding.meta.name)
        .collect();
    assert!(
        !after.contains(&"unused-var"),
        "a disabled rule should report nothing, got {after:?}"
    );
}

/// A suppression counts what it silenced, so a directive that did its job is
/// visible in the summary even though its finding is not.
#[test]
fn suppression_is_counted_rather_than_hidden() {
    let yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  # netsuke-lint: allow background-job -- deliberate\n",
        "  - name: out\n",
        "    command: \"feh preview &\"\n",
    );
    let outcome = lint(yaml);
    assert_eq!(outcome.suppressed, 1);
    assert!(
        !outcome
            .findings
            .iter()
            .any(|finding| finding.meta.name == "background-job"),
        "the suppressed finding should not be reported"
    );
}

#[test]
fn severity_counts_add_up() {
    let outcome = lint(NOISY);
    let total: usize = Severity::ALL
        .into_iter()
        .map(|severity| outcome.count_at(severity))
        .sum();
    assert_eq!(total, outcome.findings.len());
}

proptest! {
    /// Raising a rule's severity must not change which rules report, only how
    /// loudly. A rule whose detection depended on its severity would make the
    /// policy layer part of the analysis.
    #[test]
    fn severity_does_not_change_which_rules_report(
        severity in prop::sample::select(vec!["advice", "warning", "error"])
    ) {
        let policy = Policy::resolve(&[format!("redundancy={severity}")])
            .expect("selector should resolve");
        let mut reported: Vec<&str> = lint_with(NOISY, &policy)
            .findings
            .iter()
            .map(|finding| finding.meta.name)
            .collect();
        reported.sort_unstable();
        let mut baseline: Vec<&str> = lint(NOISY)
            .findings
            .iter()
            .map(|finding| finding.meta.name)
            .collect();
        baseline.sort_unstable();
        prop_assert_eq!(reported, baseline);
    }

    /// A suppression must silence only the rules it names.
    #[test]
    fn a_directive_silences_only_the_rules_it_names(
        rule in prop::sample::select(vec!["manual-ninja-escape", "literal-recipe-path", "bashism"])
    ) {
        let yaml = format!(
            concat!(
                "netsuke_version: \"1.0.0\"\n",
                "targets:\n",
                "  # netsuke-lint: allow {} -- under test\n",
                "  - name: out.txt\n",
                "    command: \"cp $$SRC out.txt; [[ -f x ]]\"\n",
            ),
            rule
        );
        let reported: Vec<&str> = lint(&yaml)
            .findings
            .iter()
            .map(|finding| finding.meta.name)
            .collect();
        prop_assert!(!reported.contains(&rule), "the named rule should be silenced");
        for other in ["manual-ninja-escape", "literal-recipe-path", "bashism"] {
            if other != rule {
                prop_assert!(
                    reported.contains(&other),
                    "`{}` should still report; got {:?}",
                    other,
                    reported
                );
            }
        }
    }
}
