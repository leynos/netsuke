//! Regression tests for YAML parse errors.
//!
//! These tests ensure diagnostics include line numbers and optional hints, and
//! that rendering is stable across terminals.

use anyhow::{Context, Result, bail, ensure};
use netsuke::manifest;
use rstest::rstest;
use strip_ansi_escapes::strip;

fn normalise_report(report: &str) -> Result<String> {
    String::from_utf8(strip(report.as_bytes())).context("YAML diagnostic should be valid UTF-8")
}

#[rstest]
#[case(
    "targets:\n\t- name: test\n",
    &[
        "line 2, column 2",
        "tabs disallowed within this context",
    ],
)]
// `serde-saphyr` 1.2.0 reports this one line earlier than 0.0.6 did: the
// offending simple key is the `command` on line 3, not the line that follows.
#[case(
    "targets:\n  - name: hi\n    command echo\n",
    &[
        "line 3, column 5",
        "simple key expected ':'",
    ],
)]
#[case(
    concat!(
        "netsuke_version: '1.0.0'\n",
        "targets:\n",
        "  - name: root\n",
        "    command: echo\n",
        "    vars:\n",
        "      nested:\n",
        "        deeper: { key: value\n",
    ),
    &[
        "line 7, column 17",
        "unclosed bracket '{'",
    ],
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
// The location moved: the unclosed quoted scalar is reported where the scanner
// detects the unterminated multi-line scalar, not at the line that opened it.
#[case(
    "targets:\n  - name: 'unterminated\n",
    &["YAML parse error", "line 3"],
)]
#[case(
    "",
    &[
        "Manifest parse failed.",
        "Manifest structure error",
        "invalid type: null, expected struct NetsukeManifest",
    ],
)]
#[case(
    "    \n    ",
    &[
        "Manifest parse failed.",
        "Manifest structure error",
        "invalid type: null, expected struct NetsukeManifest",
    ],
)]
#[case(
    "# just a comment\n# another comment",
    &[
        "Manifest parse failed.",
        "Manifest structure error",
        "invalid type: null, expected struct NetsukeManifest",
    ],
)]
// No location information should default to the start of the file.
#[case(
    "not: yaml: at all: %$#@!",
    &["YAML parse error", "line 1, column 1"],
)]
fn yaml_diagnostics_are_actionable(#[case] yaml: &str, #[case] needles: &[&str]) -> Result<()> {
    let Err(err) = manifest::from_str(yaml) else {
        bail!("parse should fail");
    };
    let msg = normalise_report(
        &err.chain()
            .map(|e: &(dyn std::error::Error + '_)| e.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    )?;
    for needle in needles {
        ensure!(msg.contains(needle), "missing: {needle}\nmessage: {msg}");
    }
    Ok(())
}
