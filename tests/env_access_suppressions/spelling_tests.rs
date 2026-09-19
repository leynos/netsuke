//! Self-tests for the spellings an attribute may be written in.
//!
//! `mask` blanks comments and literals, so what remains is code and the scan can
//! match an attribute token by token rather than by the shape of the line it
//! sits on. These tests pin the spellings that matter: `rustc` accepts each of
//! them, `#[rustfmt::skip]` freezes several past `make check-fmt`, and every one
//! of them silences the policy exactly as the canonical spelling does. Each was
//! measured against a real probe file before it was pinned here, so the set is
//! evidence rather than guesswork.
//!
//! The negative cases sit beside them because the anchor that used to exclude
//! them is gone. Prose that quotes an attribute is still not an attribute — not
//! because of where the line begins, but because quoted text never reaches the
//! matcher at all.

use super::scanner::scan_source;
use anyhow::{Result, ensure};

/// The policy lint `"(path, lint)"` pair these tests expect to see reported.
fn finding(path: &str, lint: &str) -> (String, String) {
    (String::from(path), String::from(lint))
}

/// A `#[rustfmt::skip]` freezes the spelling, so the scan must read it anyway.
///
/// This is the shape that motivated the token-wise match. `rustfmt` would join
/// the marker to its parenthesis, but the skip attribute tells it not to, and
/// `make check-fmt` then passes a file whose attribute is split. Measured: the
/// file compiles, the policy is silenced, and both `clippy` exit codes are 0.
#[test]
fn a_skipped_split_attribute_is_reported() -> Result<()> {
    let source = "#[rustfmt::skip]\n#[allow\n    (clippy::disallowed_methods, reason = \"escape hatch probe\")]\nfn probe() {}\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [finding("src/lib.rs", "clippy::disallowed_methods")],
        "expected the split attribute to be reported, got {findings:?}"
    );
    Ok(())
}

/// A newline between the marker and its parenthesis is the same attribute.
#[test]
fn a_newline_inside_the_marker_is_reported() -> Result<()> {
    let source = "#![allow(\nclippy::disallowed_methods, reason = \"escape hatch probe\")]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [finding("src/lib.rs", "clippy::disallowed_methods")],
        "expected the newline inside the marker to be read through, got {findings:?}"
    );
    Ok(())
}

/// A newline between the `#` and the `[` is legal, and silences the policy.
#[test]
fn a_newline_between_the_hash_and_the_bracket_is_reported() -> Result<()> {
    let source = "#\n[allow(warnings, reason = \"escape hatch probe\")]\nfn probe() {}\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [finding("src/lib.rs", "warnings")],
        "expected the split marker to be reported, got {findings:?}"
    );
    Ok(())
}

/// A raw identifier names the same attribute.
#[test]
fn a_raw_identifier_attribute_name_is_reported() -> Result<()> {
    let source = "#![r#allow(clippy::disallowed_methods, reason = \"escape hatch probe\")]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [finding("src/lib.rs", "clippy::disallowed_methods")],
        "expected the raw-identifier attribute to be reported, got {findings:?}"
    );
    Ok(())
}

/// A raw identifier names the same lint path.
#[test]
fn a_raw_identifier_lint_path_is_reported() -> Result<()> {
    let source = "#![allow(r#clippy::disallowed_methods, reason = \"escape hatch probe\")]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [finding("src/lib.rs", "clippy::disallowed_methods")],
        "expected the raw-identifier lint path to be reported, got {findings:?}"
    );
    Ok(())
}

/// Whitespace around a path separator does not rename the lint.
#[test]
fn spaces_around_the_path_separator_are_reported() -> Result<()> {
    let source = "#![allow(clippy :: disallowed_methods, reason = \"escape hatch probe\")]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [finding("src/lib.rs", "clippy::disallowed_methods")],
        "expected the spaced lint path to be reported, got {findings:?}"
    );
    Ok(())
}

/// The deprecated bare name is an alias, and is reported beside its enabler.
///
/// Measured twice: beside `renamed_and_removed_lints` the bare name silences
/// the policy at `clippy` exit 0, and without it the same attribute exits 101.
/// So it is the enabler that closes the class and the alias that keeps the pair
/// honest, exactly as with the path-qualified spelling.
#[test]
fn the_deprecated_bare_name_is_reported_with_its_enabler() -> Result<()> {
    let source = "#![allow(renamed_and_removed_lints, disallowed_methods, reason = \"escape hatch probe\")]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings
            == [
                finding("src/lib.rs", "renamed_and_removed_lints"),
                finding("src/lib.rs", "disallowed_methods")
            ],
        "expected both the enabler and the bare name to be reported, got {findings:?}"
    );
    Ok(())
}

/// `unknown_lints` cannot suppress the policy, so it is not banned.
///
/// It looks as though it belongs in the set — it hides the report that a name
/// does not exist — but measurement says a misspelled name is a no-op either
/// way, so allowing the report silences nothing. A rule the code cannot justify
/// is worse than an absent one; this test is what keeps the entry from being
/// added back on the strength of a plausible-sounding rationale.
#[test]
fn the_unknown_lint_enabler_is_not_a_finding_on_its_own() -> Result<()> {
    let source = "#![allow(unknown_lints, reason = \"escape hatch probe\")]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings.is_empty(),
        "expected the unknown-lint enabler to pass on its own, got {findings:?}"
    );
    Ok(())
}

/// An attribute quoted inside a line comment is prose, not code.
///
/// The line anchor used to be what excluded this; the masked text excludes it
/// now, which is what lets the matcher read attributes split across lines.
#[test]
fn an_attribute_quoted_in_a_line_comment_is_not_reported() -> Result<()> {
    let source = "let probe = 1; // #[allow(warnings, reason = \"quoted example\")]\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings.is_empty(),
        "expected a commented-out attribute to pass, got {findings:?}"
    );
    Ok(())
}

/// A blank line between the marker and its parenthesis is still one attribute.
///
/// Whitespace between tokens is not limited to a single newline, and a
/// `#[rustfmt::skip]` can hold the gap open however wide it likes.
#[test]
fn a_blank_line_before_the_parenthesis_is_reported() -> Result<()> {
    let source = "#[rustfmt::skip]\n#[allow\n\n    (clippy::disallowed_methods, reason = \"escape hatch probe\")]\nfn probe() {}\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [finding("src/lib.rs", "clippy::disallowed_methods")],
        "expected the widely split attribute to be reported, got {findings:?}"
    );
    Ok(())
}

/// A `cfg_attr` whose wrapped body holds the `allow` is read whole.
#[test]
fn a_wrapped_cfg_attr_allow_is_reported() -> Result<()> {
    let source = "#[cfg_attr(\n    all(),\n    allow(clippy::disallowed_methods, reason = \"escape hatch probe\"),\n)]\nfn probe() {}\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings == [finding("src/lib.rs", "clippy::disallowed_methods")],
        "expected the wrapped cfg_attr allow to be reported, got {findings:?}"
    );
    Ok(())
}

/// Malformed input yields findings or silence, never a panic.
///
/// The matcher reads bytes and slices at the offsets it derives from them, so
/// the shapes that could send it out of bounds — a `#` with nothing behind it,
/// a marker that never closes, a stray byte that is not a character boundary —
/// are pinned here. A panic would be a worse failure than a finding: it would
/// take the gate down rather than report it.
#[test]
fn malformed_input_is_not_a_panic() -> Result<()> {
    let cases = [
        "#",
        "#!",
        "#[",
        "#![]",
        "#[]",
        "#(",
        "#!(",
        "#[allow(",
        "#[allow(warnings",
        "#![allow(clippy::disallowed_methods",
        "#[cfg_attr(all(),",
        "#[cfg_attr(all(), allow(cfg_attr_allow_is_unterminated",
        "const C: &str = \"unterminated",
        "/* unterminated comment\n#[allow(warnings, reason = \"x\")]",
        "let \u{00e9} = 1; #[allow(warnings, reason = \"non-ascii before\")]",
        "#\u{00e9}[allow(warnings, reason = \"non-ascii after\")]",
        "#[allow(warnings, reason = \"\u{1f600}\")]",
    ];
    for case in cases {
        // The assertion is that this returns at all; each case is malformed or
        // harmless, so any finding is acceptable and a panic is not.
        let findings = scan_source("src/lib.rs", case);
        ensure!(
            findings
                .iter()
                .all(|(path, lint)| path == "src/lib.rs" && !lint.is_empty()),
            "a finding should name a path and a lint, got {findings:?} for {case:?}"
        );
    }
    Ok(())
}

/// An `#[expect]` carrying the policy lint is the sanctioned form, not an offence.
///
/// The seam taxonomy asks for an `expect` with a reason precisely so that the
/// suppression is tied to a site that still exists. The scan must not read it as
/// an `allow`, and `.expect(...)` method calls must not be read at all.
#[test]
fn an_expect_carrying_the_policy_lint_is_not_reported() -> Result<()> {
    let source = "#![expect(clippy::disallowed_methods, reason = \"sanctioned site\")]\n\
                  fn probe() {\n\
                  \x20   let value = std::env::var(\"X\");\n\
                  \x20   assert!(value.is_err());\n\
                  }\n";
    let findings = scan_source("src/lib.rs", source);

    ensure!(
        findings.is_empty(),
        "expected the sanctioned expect form to pass, got {findings:?}"
    );
    Ok(())
}
