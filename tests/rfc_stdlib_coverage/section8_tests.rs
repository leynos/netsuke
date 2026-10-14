//! Check contract citations and whole-token mentions with minimal documents.
use super::*;
use crate::rfc_stdlib_coverage::Namespace;
use rstest::rstest;

#[rstest]
#[case::no_section("# Other", Some("8.1"), "RFC 0006 has no section 8")]
#[case::no_citation(
    "## 8. Accepted capabilities\n### 8.1. Helpers\nabs",
    None,
    "name no RFC 0006 section 8 subsection"
)]
#[case::wrong_citation(
    "## 8. Accepted capabilities\n### 8.1. Helpers\nabs",
    Some("8.2"),
    "name no RFC 0006 section 8 subsection"
)]
#[case::substring(
    "## 8. Accepted capabilities\n### 8.1. Helpers\nis_abs",
    Some("8.1"),
    "never named as whole tokens"
)]
fn section_contract_failures_are_specific(
    #[case] text: &str,
    #[case] citation: Option<&str>,
    #[case] diagnostic: &str,
) {
    let accepted = BTreeMap::from([(
        "abs".into(),
        Row {
            name: "abs".into(),
            namespace: Namespace::Test,
        },
    )]);
    let sections = citation
        .map(|number| BTreeMap::from([("abs".into(), number.into())]))
        .unwrap_or_default();
    let error =
        check_section_8(&Section::whole(text), &accepted, &sections).expect_err("invalid contract");
    assert!(error.to_string().contains(diagnostic), "{error}");
}

#[rstest]
#[case::abs("abs", "is_abs")]
#[case::hash("hash", "text_hash")]
#[case::quote("quote", "shell_quote")]
#[case::difference("difference", "symmetric_difference")]
fn mentions_require_whole_identifier_tokens(#[case] name: &str, #[case] longer: &str) {
    assert!(!mentions(&[longer], name));
    assert!(mentions(&[&format!("`{name}` (value)")], name));
}

#[test]
fn subsection_scan_ignores_fenced_starts_and_peer_boundaries() {
    let text = concat!(
        "~~~\n### 8.1. Fake\n~~~\n### 8.10. Other\nwrong\n",
        "### 8.1. Real\n```\n# example\n```\nreal\n### 8.2. Peer\nother"
    );
    let lines = subsection_lines(&Section::whole(text), "8.1").expect("real section");
    assert_eq!(lines.last(), Some(&"real"));
    assert!(subsection_lines(&Section::whole(text), "8.3").is_none());
}
