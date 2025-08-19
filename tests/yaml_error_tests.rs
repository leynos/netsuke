//! Regression tests for YAML parse errors.
//!
//! These tests ensure diagnostics include line numbers and optional hints, and
//! that rendering is stable across terminals.

use miette::GraphicalReportHandler;
use netsuke::manifest;
use rstest::rstest;
use strip_ansi_escapes::strip;

fn normalise_report(report: &str) -> String {
    String::from_utf8(strip(report.as_bytes())).expect("utf8")
}

#[rstest]
#[case(
    "targets:\n\t- name: test\n",
    &["line 2, column 1", "Use spaces for indentation"],
)]
#[case(
    "targets:\n  - name: hi\n    command echo\n",
    &["line 3", "expected ':'", "Ensure each key is followed by ':'"],
)]
#[case(
    concat!(
        "targets:\n",
        "  - name: ok\n",
        "    command: echo\n",
        "  name: missing\n",
        "    command: echo\n",
    ),
    &["line 4", "did not find expected '-'", "Start list items with '-'"],
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
    "not: yaml: at all: %$#@!",
    &["YAML parse error", "line 1"],
)]
fn yaml_diagnostics_are_actionable(#[case] yaml: &str, #[case] needles: &[&str]) {
    let err = manifest::from_str(yaml).expect_err("parse should fail");
    let mut msg = String::new();
    GraphicalReportHandler::new()
        .render_report(&mut msg, err.as_ref())
        .expect("render yaml error");
    let msg = normalise_report(&msg);
    for needle in needles {
        assert!(msg.contains(needle), "missing: {needle}\nmessage: {msg}");
    }
}
