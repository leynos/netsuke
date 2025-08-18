use netsuke::manifest;

#[test]
fn reports_line_and_column_with_tab_hint() {
    let yaml = "targets:\n\t- name: test\n";
    let err = manifest::from_str(yaml).expect_err("parse should fail");
    let msg = err.to_string();
    assert!(msg.contains("line 2, column 1"), "missing location: {msg}");
    assert!(
        msg.contains("Use spaces for indentation"),
        "missing hint: {msg}"
    );
}

#[test]
fn suggests_colon_when_missing() {
    let yaml = "targets:\n  - name: hi\n    command echo\n";
    let err = manifest::from_str(yaml).expect_err("parse should fail");
    let msg = err.to_string();
    assert!(msg.contains("line 3"), "missing line info: {msg}");
    assert!(msg.contains("expected ':'"), "missing error detail: {msg}");
    assert!(
        msg.contains("Ensure each key is followed by ':'"),
        "missing suggestion: {msg}"
    );
}
