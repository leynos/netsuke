use miette::GraphicalReportHandler;
use netsuke::manifest;
use rstest::rstest;

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
    "targets:\n  - command: [echo\n",
    &["line 2", "did not find expected ',' or ']'"],
)]
fn yaml_diagnostics_are_actionable(#[case] yaml: &str, #[case] needles: &[&str]) {
    let err = manifest::from_str(yaml).expect_err("parse should fail");
    let diag = err
        .downcast_ref::<manifest::YamlDiagnostic>()
        .expect("diagnostic type");
    let mut msg = String::new();
    GraphicalReportHandler::new()
        .render_report(&mut msg, diag)
        .expect("render yaml error");
    for needle in needles {
        assert!(msg.contains(needle), "missing: {needle}\nmessage: {msg}");
    }
}
