//! Regression tests for YAML parse errors.
//!
//! These tests ensure diagnostics include line numbers and optional hints, and
//! that rendering is stable across terminals.

use netsuke::manifest;
use rstest::rstest;
use strip_ansi_escapes::strip;

fn normalise_report(report: &str) -> String {
    String::from_utf8(strip(report.as_bytes())).expect("utf8")
}

#[rstest]
#[case(
    "targets:\n\t- name: test\n",
    &[
        "line 2, column 1",
        "Use spaces for indentation; tabs are invalid in YAML.",
    ],
)]
#[case(
    "targets:\n  - name: hi\n    command echo\n",
    &["line 3", "expected ':'"],
)]
#[case(
    concat!(
        "targets:\n",
        "  - name: ok\n",
        "    command: echo\n",
        "  name: missing\n",
        "    command: echo\n",
    ),
    &["line 4", "did not find expected '-'"] ,
)]
#[case(
    "targets:\n  - name: 'unterminated\n",
    &["YAML parse error", "line 2"],
)]
#[case(
    "",
    &[
        "manifest parse error",
        "missing field",
        "netsuke_version",
    ],
)]
#[case(
    "    \n    ",
    &[
        "manifest parse error",
        "missing field",
        "netsuke_version",
    ],
)]
#[case(
    "# just a comment\n# another comment",
    &[
        "manifest parse error",
        "missing field",
        "netsuke_version",
    ],
)]
// No location information should default to the start of the file.
#[case(
    "not: yaml: at all: %$#@!",
    &["YAML parse error", "line 1, column 1"],
)]
fn yaml_diagnostics_are_actionable(#[case] yaml: &str, #[case] needles: &[&str]) {
    let err = manifest::from_str(yaml).expect_err("parse should fail");
    let msg = normalise_report(
        &err.chain()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    );
    for needle in needles {
        assert!(msg.contains(needle), "missing: {needle}\nmessage: {msg}");
    }
}
