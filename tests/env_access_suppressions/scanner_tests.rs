//! Self-tests for the suppression scanner.
//!
//! Each test pins one shape the scan must classify: a suppression it must
//! report, or an innocent source it must not. They run against synthetic text
//! rather than the repository, so the gate's own behaviour is asserted directly
//! instead of being inferred from a clean walk over sources that happen to be
//! clean.

use super::scanner::scan_source;
use anyhow::{Result, ensure};

/// An inner `allow` of the policy lint is the evasion this contract exists for.
#[test]
fn inner_allow_of_the_policy_lint_is_reported() -> Result<()> {
    let source = "#![allow(clippy::disallowed_methods, reason = \"escape hatch probe\")]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings
            == [(
                String::from("src/lib.rs"),
                String::from("clippy::disallowed_methods")
            )],
        "expected the inner allow to be reported, got {findings:?}"
    );
    Ok(())
}

/// An item-level `allow` of the blanket spelling is reported too.
#[test]
fn item_allow_of_warnings_is_reported() -> Result<()> {
    let source = "#[allow(warnings, reason = \"escape hatch probe\")]\nfn probe() {}\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [(String::from("src/lib.rs"), String::from("warnings"))],
        "expected the item-level allow to be reported, got {findings:?}"
    );
    Ok(())
}

/// The group above the policy lint silences it just as naming it does.
#[test]
fn allow_of_the_enclosing_group_is_reported() -> Result<()> {
    let source = "#![allow(clippy::style, reason = \"escape hatch probe\")]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [(String::from("src/lib.rs"), String::from("clippy::style"))],
        "expected the enclosing group to be reported, got {findings:?}"
    );
    Ok(())
}

/// A wrapped attribute is read whole, as `rustfmt` writes a long one.
#[test]
fn wrapped_attribute_is_read_whole() -> Result<()> {
    let source =
        "#![allow(\n    clippy::disallowed_methods,\n    reason = \"escape hatch probe\"\n)]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings
            == [(
                String::from("src/lib.rs"),
                String::from("clippy::disallowed_methods")
            )],
        "expected the wrapped allow to be reported, got {findings:?}"
    );
    Ok(())
}

/// A `)` inside the reason string does not end the attribute early.
#[test]
fn parenthesis_inside_a_reason_does_not_end_the_attribute() -> Result<()> {
    let source = "#[allow(warnings, reason = \"closing ) paren\")]\nfn probe() {}\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [(String::from("src/lib.rs"), String::from("warnings"))],
        "expected the allow to be read to its matching parenthesis, got {findings:?}"
    );
    Ok(())
}

/// Suppression in general is not the offence: an unrelated lint still passes.
#[test]
fn unrelated_allow_is_not_reported() -> Result<()> {
    let source = "#![allow(dead_code, reason = \"shared test-support module\")]\n";
    let findings = scan_source("test_support/src/lib.rs", source);

    ensure!(
        findings.is_empty(),
        "expected an unrelated allow to pass, got {findings:?}"
    );
    Ok(())
}

/// The exemption covers the derive-isolation modules and only those.
#[test]
fn guard_lint_exemption_is_scoped_to_the_isolation_modules() -> Result<()> {
    let source = "#![allow(\n    clippy::allow_attributes,\n    clippy::allow_attributes_without_reason,\n    unused_assignments\n)]\n";
    let findings = scan_source("src/runner/error.rs", source);

    ensure!(
        findings.is_empty(),
        "expected the documented exemption to pass, got {findings:?}"
    );
    Ok(())
}

/// The same attribute elsewhere still names the policy-carrying guard lints.
#[test]
fn guard_lint_exemption_does_not_cover_other_paths() -> Result<()> {
    let source = "#![allow(\n    clippy::allow_attributes,\n    clippy::allow_attributes_without_reason,\n    unused_assignments\n)]\n";
    let findings = scan_source("src/elsewhere.rs", source);

    ensure!(
        findings
            == [
                (
                    String::from("src/elsewhere.rs"),
                    String::from("clippy::allow_attributes")
                ),
                (
                    String::from("src/elsewhere.rs"),
                    String::from("clippy::allow_attributes_without_reason")
                )
            ],
        "expected the guard lints to be reported off the exempt paths, got {findings:?}"
    );
    Ok(())
}

/// An attribute-looking line inside a block comment suppresses nothing.
#[test]
fn attribute_inside_a_block_comment_is_not_reported() -> Result<()> {
    let source = "/*\n#[allow(clippy::disallowed_methods, reason = \"commented example\")]\n*/\npub fn probe() {}\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings.is_empty(),
        "expected a commented-out attribute to pass, got {findings:?}"
    );
    Ok(())
}

/// An attribute-looking line inside a raw string suppresses nothing.
#[test]
fn attribute_inside_a_raw_string_is_not_reported() -> Result<()> {
    let source =
        "const FIXTURE: &str = r#\"\n#[allow(warnings, reason = \"quoted example\")]\n\"#;\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings.is_empty(),
        "expected an attribute quoted inside a string to pass, got {findings:?}"
    );
    Ok(())
}

/// An escaped quote in a byte string does not hide the attribute after it.
#[test]
fn an_escaped_quote_in_a_byte_string_does_not_hide_a_later_attribute() -> Result<()> {
    let source = r#"fn probe() { let sample = b"a \" b"; }
#[allow(clippy::disallowed_methods, reason = "escape hatch probe")]
fn probe2() {}
"#;
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings
            == [(
                String::from("src/lib.rs"),
                String::from("clippy::disallowed_methods")
            )],
        "expected the byte string to close at its own terminator and the attribute after it \
         to be reported, got {findings:?}"
    );
    Ok(())
}

/// An attribute-looking line inside a byte string is quoted text, not code.
#[test]
fn attribute_inside_a_byte_string_is_not_reported() -> Result<()> {
    let source = r#"const SAMPLE: &[u8] = b"one \" two
#[allow(warnings, reason = \"sample\")]
three";
"#;
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings.is_empty(),
        "expected an attribute quoted inside a byte string to pass, got {findings:?}"
    );
    Ok(())
}

/// A lifetime is not an unterminated char literal that blanks the code after it.
#[test]
fn a_lifetime_does_not_blank_the_attribute_that_follows() -> Result<()> {
    let source =
        "fn probe<'a>(value: &'a str) {}\n#[allow(warnings, reason = \"escape hatch probe\")]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [(String::from("src/lib.rs"), String::from("warnings"))],
        "expected the attribute after a lifetime to be reported, got {findings:?}"
    );
    Ok(())
}

/// A `cfg_attr`-wrapped allow suppresses the policy just as a direct one does.
#[test]
fn cfg_attr_wrapped_allow_is_reported() -> Result<()> {
    let source =
        "#![cfg_attr(all(), allow(clippy::disallowed_methods, reason = \"escape hatch probe\"))]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings
            == [(
                String::from("src/lib.rs"),
                String::from("clippy::disallowed_methods")
            )],
        "expected the cfg_attr-wrapped allow to be reported, got {findings:?}"
    );
    Ok(())
}

/// A `cfg_attr` naming a lint but not suppressing it is not an offence.
#[test]
fn cfg_attr_without_an_allow_is_not_reported() -> Result<()> {
    let source = "#![cfg_attr(test, deny(clippy::disallowed_methods))]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings.is_empty(),
        "expected a cfg_attr that denies rather than allows to pass, got {findings:?}"
    );
    Ok(())
}

/// An `allow` nested several `cfg_attr`s deep is still read.
#[test]
fn nested_cfg_attr_allow_is_reported() -> Result<()> {
    let source = "#[cfg_attr(all(), cfg_attr(all(), allow(clippy::style, reason = \"escape hatch probe\")))]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [(String::from("src/lib.rs"), String::from("clippy::style"))],
        "expected the nested cfg_attr allow to be reported, got {findings:?}"
    );
    Ok(())
}

/// A group name containing `allow` is not itself an `allow` attribute.
#[test]
fn a_lint_name_containing_allow_is_not_a_nested_attribute() -> Result<()> {
    let source =
        "#[cfg_attr(all(), expect(clippy::allow_attributes, reason = \"probe\"))]\nfn probe() {}\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings.is_empty(),
        "expected a foreign lint name to pass, got {findings:?}"
    );
    Ok(())
}
