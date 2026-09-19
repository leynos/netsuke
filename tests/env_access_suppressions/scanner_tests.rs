//! Self-tests for the suppression scanner.
//!
//! Each row of the table below pins one shape the scan must classify: a
//! suppression it must report, or an innocent source it must not. They run
//! against synthetic text rather than the repository, so the gate's own
//! behaviour is asserted directly instead of being inferred from a clean walk
//! over sources that happen to be clean.
//!
//! The rows share one function because they share one assertion: the findings
//! for a source equal the lints listed for it, and an innocent shape is a row
//! whose list is empty. Split into separate tests the two directions could
//! drift apart, and a shape that stopped being reported would look like a shape
//! that was never expected to be. Each case is named for the shape it pins, so
//! a failure names the shape rather than a row number.

use super::expected_findings;
use super::scanner::scan_source;
use anyhow::{Result, ensure};
use rstest::rstest;

/// Every shape the scan must classify, as path, source, and expected lints.
///
/// An empty list means the source suppresses nothing and the scan must find
/// nothing in it. The path varies because it is not merely a label: the scoped
/// exemption is keyed on it, and two rows below pin that the exemption covers
/// the derive-isolation modules and nothing else.
#[rstest]
// The evasion this contract exists for: an inner attribute, at the top of a
// file, that switches the policy off for everything below it.
#[case::inner_allow(
    "src/lib.rs",
    "#![allow(clippy::disallowed_methods, reason = \"escape hatch probe\")]\n",
    &["clippy::disallowed_methods"]
)]
// The item-level form, and the blanket spelling a reader reaches for first.
#[case::item_allow_of_warnings(
    "src/lib.rs",
    "#[allow(warnings, reason = \"escape hatch probe\")]\nfn probe() {}\n",
    &["warnings"]
)]
// The group above the policy lint silences it just as naming it does.
#[case::enclosing_group(
    "src/lib.rs",
    "#![allow(clippy::style, reason = \"escape hatch probe\")]\n",
    &["clippy::style"]
)]
// `clippy::restriction` is banned for a different reason than the groups above:
// it does not reach the policy lint at all. It is the group of the two guard
// lints, so one crate-level attribute silences the reporter and an item-level
// bare `allow` further down then passes unreported. Measured: exit 101 with no
// crate attribute, exit 0 with this one. This row pins the entry, which a
// reader measuring only against the policy lint would otherwise remove.
#[case::guard_lint_group(
    "src/lib.rs",
    "#![allow(clippy::restriction, reason = \"escape hatch probe\")]\n",
    &["clippy::restriction"]
)]
// A wrapped attribute is read whole, as `rustfmt` writes a long one.
#[case::wrapped(
    "src/lib.rs",
    "#![allow(\n    clippy::disallowed_methods,\n    reason = \"escape hatch probe\"\n)]\n",
    &["clippy::disallowed_methods"]
)]
// A `)` inside the reason string does not end the attribute early.
#[case::parenthesis_in_the_reason(
    "src/lib.rs",
    "#[allow(warnings, reason = \"closing ) paren\")]\nfn probe() {}\n",
    &["warnings"]
)]
// The old spelling of the policy lint still selects it, so it is banned too.
#[case::renamed_spelling(
    "src/lib.rs",
    "#![allow(clippy::disallowed_method, reason = \"escape hatch probe\")]\n",
    &["clippy::disallowed_method"]
)]
// The enabler that hides the rename is the ingredient that makes it silent.
#[case::rename_enabler_with_alias(
    "src/lib.rs",
    "#![allow(\n    renamed_and_removed_lints,\n    clippy::disallowed_method,\n    reason = \"escape hatch probe\"\n)]\n",
    &["renamed_and_removed_lints", "clippy::disallowed_method"]
)]
// A `cfg_attr`-wrapped allow suppresses the policy just as a direct one does.
#[case::cfg_attr_wrapped(
    "src/lib.rs",
    "#![cfg_attr(all(), allow(clippy::disallowed_methods, reason = \"escape hatch probe\"))]\n",
    &["clippy::disallowed_methods"]
)]
// An `allow` nested several `cfg_attr`s deep is still read.
#[case::nested_cfg_attr(
    "src/lib.rs",
    "#[cfg_attr(all(), cfg_attr(all(), allow(clippy::style, reason = \"escape hatch probe\")))]\n",
    &["clippy::style"]
)]
// An inner attribute inside a macro body is read, wherever it sits.
#[case::inner_attribute_in_a_macro_body(
    "src/lib.rs",
    "macro_rules! probe_macro {\n    () => {\n        #![allow(clippy::disallowed_methods, reason = \"escape hatch probe\")]\n    };\n}\n",
    &["clippy::disallowed_methods"]
)]
// A guard lint named off an exempt path is a finding. Nothing else reports a
// crate that has silenced the reporter, so the scan is the only thing standing
// between this attribute and a silenced `allow_attributes`.
#[case::path_segment_named_allow(
    "src/lib.rs",
    "#![allow(clippy::allow_attributes, reason = \"escape hatch probe\")]\n",
    &["clippy::allow_attributes"]
)]
// A lifetime is not an unterminated char literal that blanks the code after it.
#[case::attribute_after_a_lifetime(
    "src/lib.rs",
    "fn probe<'a>(value: &'a str) {}\n#[allow(warnings, reason = \"escape hatch probe\")]\n",
    &["warnings"]
)]
// Suppression in general is not the offence: an unrelated lint still passes.
#[case::unrelated_allow(
    "test_support/src/lib.rs",
    "#![allow(dead_code, reason = \"shared test-support module\")]\n",
    &[]
)]
// The exemption covers the derive-isolation modules ...
#[case::exempt_isolation_module(
    "src/runner/error.rs",
    "#![allow(\n    clippy::allow_attributes,\n    clippy::allow_attributes_without_reason,\n    unused_assignments\n)]\n",
    &[]
)]
// ... and only those: the same attribute elsewhere still names the guard lints.
#[case::guard_lints_off_the_exempt_path(
    "src/elsewhere.rs",
    "#![allow(\n    clippy::allow_attributes,\n    clippy::allow_attributes_without_reason,\n    unused_assignments\n)]\n",
    &[
        "clippy::allow_attributes",
        "clippy::allow_attributes_without_reason"
    ]
)]
// An attribute-looking line inside a block comment suppresses nothing.
#[case::attribute_in_a_block_comment(
    "src/lib.rs",
    "/*\n#[allow(clippy::disallowed_methods, reason = \"commented example\")]\n*/\npub fn probe() {}\n",
    &[]
)]
// An attribute-looking line inside a raw string suppresses nothing.
#[case::attribute_in_a_raw_string(
    "src/lib.rs",
    "const FIXTURE: &str = r#\"\n#[allow(warnings, reason = \"quoted example\")]\n\"#;\n",
    &[]
)]
// An escaped quote in a byte string does not hide the attribute after it.
#[case::escaped_quote_in_a_byte_string(
    "src/lib.rs",
    r#"fn probe() { let sample = b"a \" b"; }
#[allow(clippy::disallowed_methods, reason = "escape hatch probe")]
fn probe2() {}
"#,
    &["clippy::disallowed_methods"]
)]
// An attribute-looking line inside a byte string is quoted text, not code.
#[case::attribute_in_a_byte_string(
    "src/lib.rs",
    r#"const SAMPLE: &[u8] = b"one \" two
#[allow(warnings, reason = \"sample\")]
three";
"#,
    &[]
)]
// A `cfg_attr` naming a lint but not suppressing it is not an offence.
#[case::cfg_attr_denies(
    "src/lib.rs",
    "#![cfg_attr(test, deny(clippy::disallowed_methods))]\n",
    &[]
)]
// A group name containing `allow` is not itself an `allow` attribute.
#[case::lint_name_containing_allow(
    "src/lib.rs",
    "#[cfg_attr(all(), expect(clippy::allow_attributes, reason = \"probe\"))]\nfn probe() {}\n",
    &[]
)]
fn the_scan_reports_the_expected_lints(
    #[case] path: &str,
    #[case] source: &str,
    #[case] expected_lints: &[&str],
) -> Result<()> {
    let findings = scan_source(path, source);
    ensure!(
        findings == expected_findings(path, expected_lints),
        "expected {expected_lints:?} in {path}, got {findings:?}"
    );
    Ok(())
}
