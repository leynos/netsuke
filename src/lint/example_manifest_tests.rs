//! The linter's findings for the repository's own example manifests.
//!
//! These pin the rules against real input rather than fixtures written to
//! satisfy them, and they fail if an example regresses or if a rule starts
//! reporting something new about one.

use rstest::rstest;

use crate::lint::test_support::{lint, lint_with};
use crate::lint::{Category, Policy};

/// Lint an example manifest and report each finding as `rule: message`.
fn findings_for(path: &str) -> Vec<String> {
    let text = std::fs::read_to_string(path).expect("example manifest should be readable");
    lint(&text)
        .findings
        .iter()
        .map(|finding| format!("{}: {}", finding.meta.name, finding.display_message()))
        .collect()
}

#[rstest]
#[case::basic_c(
    "examples/basic_c.yml",
    &[
        "action-without-description: action `run` is not described in `netsuke help targets`",
        "action-without-description: action `clean` is not described in `netsuke help targets`",
        "builtin-clean-action: action `clean` duplicates the built-in `netsuke clean`",
    ]
)]
#[case::photo_edit(
    "examples/photo_edit.yml",
    &[
        "action-without-description: action `preview` is not described in `netsuke help targets`",
        "background-job: action `preview` `script` detaches a process",
    ]
)]
#[case::visual_design(
    "examples/visual_design.yml",
    &[
        "action-without-description: action `clean` is not described in `netsuke help targets`",
        "builtin-clean-action: action `clean` duplicates the built-in `netsuke clean`",
    ]
)]
#[case::website("examples/website.yml", &[])]
#[case::writing(
    "examples/writing.yml",
    &[
        "command-chain-not-list: rule `combine` chains 2 steps in one `command` scalar",
        "directory-dep-not-order-only: target `build/architecture.tex`: \
         depends on the directory `build` through `deps`",
        "directory-dep-not-order-only: target `build/introduction.tex`: \
         depends on the directory `build` through `deps`",
    ]
)]
#[case::hello_world(
    "examples/hello-world/Netsukefile",
    &[
        "literal-recipe-path: target `output.txt` `command` repeats the declared path \
         `input.txt`; use `{{ ins }}`",
        "literal-recipe-path: target `output.txt` `command` repeats the declared path \
         `output.txt`; use `{{ outs }}`",
        "literal-recipe-path: target `greeting.txt` `command` repeats the declared path \
         `greeting.txt`; use `{{ outs }}`",
    ]
)]
fn examples_report_their_known_defects(#[case] path: &str, #[case] expected: &[&str]) {
    assert_eq!(findings_for(path), expected, "findings for {path}");
}

/// Every example must lint cleanly once its findings are silenced by policy.
///
/// This guards the engine rather than the rules: a rule that reported without
/// being enabled, or that ignored a disabled category, would show up here.
#[test]
fn disabling_every_category_reports_nothing() {
    let selectors: Vec<String> = Category::ALL
        .into_iter()
        .map(|category| format!("{}=off", category.as_str()))
        .collect();
    let policy = Policy::resolve(&selectors).expect("selectors should resolve");
    for path in [
        "examples/basic_c.yml",
        "examples/writing.yml",
        "examples/hello-world/Netsukefile",
    ] {
        let text = std::fs::read_to_string(path).expect("example manifest should be readable");
        let outcome = lint_with(&text, &policy);
        assert!(
            outcome.findings.is_empty(),
            "{path} should report nothing, got {:?}",
            outcome
                .findings
                .iter()
                .map(|finding| finding.meta.name)
                .collect::<Vec<_>>()
        );
    }
}
