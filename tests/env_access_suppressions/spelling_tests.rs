//! Self-tests for the spellings an attribute may be written in.
//!
//! `mask` blanks comments and literals, so what remains is code and the scan can
//! match an attribute token by token rather than by the shape of the line it
//! sits on. These cases pin the spellings that matter: `rustc` accepts each of
//! them, `#[rustfmt::skip]` freezes several past `make check-fmt`, and every one
//! of them silences the policy exactly as the canonical spelling does. Each was
//! measured against a real probe file before it was pinned here, so the set is
//! evidence rather than guesswork.
//!
//! The negative cases sit beside them because the anchor that used to exclude
//! them is gone. Prose that quotes an attribute is still not an attribute — not
//! because of where the line begins, but because quoted text never reaches the
//! matcher at all.
//!
//! `malformed_input_is_not_a_panic` stays a test of its own: it asserts
//! something weaker than the rest — that the matcher returns at all — over
//! inputs that are not shapes so much as the absence of one.

use super::expected_findings;
use super::scanner::scan_source;
use anyhow::{Result, ensure};
use rstest::rstest;

/// The evading spellings, and the innocent ones the anchor used to exclude.
///
/// An empty list means the source is not an offence. The raw-identifier rows
/// are the dangerous group: they need no `#[rustfmt::skip]`, so `rustfmt`
/// leaves them byte-for-byte and they were reachable on a clean
/// `make check-fmt` run even while the anchor stood.
#[rstest]
// A `#[rustfmt::skip]` freezes the spelling, so the scan must read it anyway.
// `rustfmt` would join the marker to its parenthesis, but the skip attribute
// tells it not to, and `make check-fmt` then passes a file whose attribute is
// split. Measured: the file compiles, the policy is silenced, and both clippy
// exit codes are 0.
#[case::skipped_split_attribute(
    "#[rustfmt::skip]\n#[allow\n    (clippy::disallowed_methods, reason = \"escape hatch probe\")]\nfn probe() {}\n",
    &["clippy::disallowed_methods"]
)]
// A newline between the marker and its parenthesis is the same attribute.
#[case::newline_inside_the_marker(
    "#![allow(\nclippy::disallowed_methods, reason = \"escape hatch probe\")]\n",
    &["clippy::disallowed_methods"]
)]
// A newline between the `#` and the `[` is legal, and silences the policy.
#[case::newline_before_the_bracket(
    "#\n[allow(warnings, reason = \"escape hatch probe\")]\nfn probe() {}\n",
    &["warnings"]
)]
// A blank line between the marker and its parenthesis is still one attribute.
// Whitespace between tokens is not limited to a single newline, and a
// `#[rustfmt::skip]` can hold the gap open however wide it likes.
#[case::blank_line_before_the_parenthesis(
    "#[rustfmt::skip]\n#[allow\n\n    (clippy::disallowed_methods, reason = \"escape hatch probe\")]\nfn probe() {}\n",
    &["clippy::disallowed_methods"]
)]
// A raw identifier names the same attribute.
#[case::raw_identifier_attribute_name(
    "#![r#allow(clippy::disallowed_methods, reason = \"escape hatch probe\")]\n",
    &["clippy::disallowed_methods"]
)]
// A raw identifier names the same lint path.
#[case::raw_identifier_lint_path(
    "#![allow(r#clippy::disallowed_methods, reason = \"escape hatch probe\")]\n",
    &["clippy::disallowed_methods"]
)]
// Whitespace around a path separator does not rename the lint.
#[case::spaces_around_the_path_separator(
    "#![allow(clippy :: disallowed_methods, reason = \"escape hatch probe\")]\n",
    &["clippy::disallowed_methods"]
)]
// The deprecated bare name is an alias, and is reported beside its enabler.
// Measured twice: beside `renamed_and_removed_lints` the bare name silences the
// policy at clippy exit 0, and without it the same attribute exits 101. So it
// is the enabler that closes the class and the alias that keeps the pair
// honest, exactly as with the path-qualified spelling.
#[case::deprecated_bare_name_with_enabler(
    "#![allow(renamed_and_removed_lints, disallowed_methods, reason = \"escape hatch probe\")]\n",
    &["renamed_and_removed_lints", "disallowed_methods"]
)]
// `unknown_lints` cannot suppress the policy, so it is not banned. It looks as
// though it belongs in the set — it hides the report that a name does not exist
// — but measurement says a misspelled name is a no-op either way, so allowing
// the report silences nothing. A rule the code cannot justify is worse than an
// absent one; this row is what keeps the entry from being added back on the
// strength of a plausible-sounding rationale.
#[case::unknown_lints_enabler_alone(
    "#![allow(unknown_lints, reason = \"escape hatch probe\")]\n",
    &[]
)]
// An attribute quoted inside a line comment is prose, not code. The line anchor
// used to be what excluded this; the masked text excludes it now, which is what
// lets the matcher read attributes split across lines.
#[case::attribute_in_a_line_comment(
    "let probe = 1; // #[allow(warnings, reason = \"quoted example\")]\n",
    &[]
)]
// A `cfg_attr` whose wrapped body holds the `allow` is read whole.
#[case::wrapped_cfg_attr_allow(
    "#[cfg_attr(\n    all(),\n    allow(clippy::disallowed_methods, reason = \"escape hatch probe\"),\n)]\nfn probe() {}\n",
    &["clippy::disallowed_methods"]
)]
// An `#[expect]` carrying the policy lint is the sanctioned form, not an
// offence. The seam taxonomy asks for an `expect` with a reason precisely so
// that the suppression is tied to a site that still exists. The scan must not
// read it as an `allow`, and `.expect(...)` method calls must not be read at
// all.
#[case::expect_carrying_the_policy_lint(
    "#![expect(clippy::disallowed_methods, reason = \"sanctioned site\")]\n\
     fn probe() {\n\
     \x20   let value = std::env::var(\"X\");\n\
     \x20   assert!(value.is_err());\n\
     }\n",
    &[]
)]
// A `warn` of the policy lint is not read, and this row is what pins that the
// omission is deliberate. It lowers the lint from the workspace's `deny` to
// `warn` — bare `cargo clippy` exits 0 — but every lint target passes
// `-D warnings`, which re-promotes it: measured at exit 101 under the gate's
// flags. Reporting a shape that cannot pass a gate would be a rule the code
// cannot justify, the same reasoning that leaves `unknown_lints` out of the
// banned set.
#[case::warn_of_the_policy_lint_alone(
    "#![warn(clippy::disallowed_methods)]\n",
    &[]
)]
// ... and the same holds for the item-level and `cfg_attr`-wrapped spellings,
// so the omission is about the `warn` marker rather than one layout.
#[case::warn_wrapped_in_a_cfg_attr(
    "#![cfg_attr(all(), warn(clippy::disallowed_methods))]\n",
    &[]
)]
// The pair that *does* escape the gate's flags, and the reason `warnings`
// stays in the banned set. The `warn` lowers the policy lint to `warn`, which
// is what puts it *into* the `warnings` group — the group is the set of lints
// currently at `warn`, not a parent of the hierarchy — and the `allow` then
// suppresses that group. Measured at exit 0 under `RUSTFLAGS=-D warnings`, in
// either order, where neither half escapes alone. The scan catches it on the
// `allow` half, which is the only half it can see.
#[case::warn_of_the_policy_lint_beside_allow_warnings(
    "#![warn(clippy::disallowed_methods)]\n\
     #![allow(warnings, reason = \"escape hatch probe\")]\n",
    &["warnings"]
)]
fn the_scan_reads_each_spelling_the_same_way(
    #[case] source: &str,
    #[case] expected_lints: &[&str],
) -> Result<()> {
    let findings = scan_source("src/lib.rs", source);
    ensure!(
        findings == expected_findings("src/lib.rs", expected_lints),
        "expected {expected_lints:?}, got {findings:?} for {source:?}"
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
