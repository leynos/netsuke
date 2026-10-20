//! Check the selector's JSON/YAML contract and parser compatibility boundary.

use super::ShellSelection;
use crate::shell_selection::ShellNameError;
use pretty_assertions::assert_eq;
use rstest::rstest;
use serde::Deserialize;
use serde_json::{Value, json};

/// Record the semantic class expected from a selector decoder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExpectedSelection {
    /// Expect direct execution.
    Direct,
    /// Expect the platform default.
    PlatformDefault,
    /// Expect the named shell spelling.
    Named(&'static str),
    /// Expect decoding to reject the input.
    Rejected,
}

/// Decode a selector into a small probe for absent-key default tests.
#[derive(Debug, Deserialize)]
struct Probe {
    /// Capture the selector while defaulting an omitted mapping key.
    #[serde(default)]
    shell: ShellSelection,
}

/// Check a decoded value against its expected semantic class.
fn matches_expected(outcome: &Result<ShellSelection, String>, expected: ExpectedSelection) -> bool {
    match (outcome, expected) {
        (Ok(ShellSelection::Direct), ExpectedSelection::Direct)
        | (Ok(ShellSelection::PlatformDefault), ExpectedSelection::PlatformDefault)
        | (Err(_), ExpectedSelection::Rejected) => true,
        (Ok(ShellSelection::Named(name)), ExpectedSelection::Named(expected_name)) => {
            name.as_str() == expected_name
        }
        _ => false,
    }
}

#[rstest]
#[case(json!(false), ExpectedSelection::Direct)]
#[case(json!(true), ExpectedSelection::PlatformDefault)]
#[case(json!("bash"), ExpectedSelection::Named("bash"))]
#[case(json!("null"), ExpectedSelection::Named("null"))]
fn decodes_supported_json_values(#[case] value: Value, #[case] expected: ExpectedSelection) {
    let actual = serde_json::from_value::<ShellSelection>(value);
    assert!(
        matches_expected(&actual.map_err(|error| error.to_string()), expected),
        "supported selector must decode as {expected:?}"
    );
}

#[rstest]
#[case(Value::Null)]
#[case(json!(1))]
#[case(json!(1.5))]
#[case(json!([]))]
#[case(json!({"name": "bash"}))]
#[case(json!("Bash"))]
#[case(json!("/bin/bash"))]
#[case(json!("true"))]
fn rejects_unsupported_json_values(#[case] value: Value) {
    assert!(
        serde_json::from_value::<ShellSelection>(value).is_err(),
        "unsupported selector must be rejected"
    );
}

#[test]
fn defaults_to_direct_execution() {
    assert_eq!(ShellSelection::default(), ShellSelection::Direct);
}

#[rstest]
#[case("{}\n", ExpectedSelection::Direct, true)]
#[case("shell: false\n", ExpectedSelection::Direct, true)]
#[case("shell: False\n", ExpectedSelection::Direct, true)]
#[case("shell: no\n", ExpectedSelection::Direct, true)]
#[case("shell: n\n", ExpectedSelection::Direct, true)]
#[case("shell: N\n", ExpectedSelection::Direct, true)]
#[case("shell: off\n", ExpectedSelection::Direct, true)]
#[case("shell: oFf\n", ExpectedSelection::Direct, true)]
#[case("shell: true\n", ExpectedSelection::PlatformDefault, true)]
#[case("shell: TRUE\n", ExpectedSelection::PlatformDefault, true)]
#[case("shell: yes\n", ExpectedSelection::PlatformDefault, true)]
#[case("shell: yEs\n", ExpectedSelection::PlatformDefault, true)]
#[case("shell: y\n", ExpectedSelection::PlatformDefault, true)]
#[case("shell: Y\n", ExpectedSelection::PlatformDefault, true)]
#[case("shell: on\n", ExpectedSelection::PlatformDefault, true)]
#[case("shell: ON\n", ExpectedSelection::PlatformDefault, true)]
#[case("shell: yes\u{00a0}\n", ExpectedSelection::PlatformDefault, true)]
#[case("shell: bash\n", ExpectedSelection::Named("bash"), true)]
#[case("shell: dash\n", ExpectedSelection::Named("dash"), true)]
#[case("shell: \"bash\"\n", ExpectedSelection::Named("bash"), true)]
#[case("shell: 'dash'\n", ExpectedSelection::Named("dash"), true)]
#[case("shell: \"null\"\n", ExpectedSelection::Named("null"), true)]
#[case("shell: \"true\"\n", ExpectedSelection::Rejected, true)]
#[case("shell: \"false\"\n", ExpectedSelection::Rejected, true)]
#[case("shell: 'yes'\n", ExpectedSelection::Rejected, true)]
#[case("shell: \"n\"\n", ExpectedSelection::Rejected, true)]
#[case("shell: \"off\"\n", ExpectedSelection::Rejected, true)]
#[case("shell:\n", ExpectedSelection::Rejected, true)]
#[case("shell: ~\n", ExpectedSelection::Rejected, true)]
#[case("shell: null\n", ExpectedSelection::Rejected, true)]
#[case("shell: Null\n", ExpectedSelection::Rejected, true)]
#[case("shell: 1\n", ExpectedSelection::Rejected, true)]
#[case("shell: 0x10\n", ExpectedSelection::Rejected, true)]
#[case("shell: \"\"\n", ExpectedSelection::Rejected, true)]
#[case("shell: [bash]\n", ExpectedSelection::Rejected, true)]
#[case("shell: {name: bash}\n", ExpectedSelection::Rejected, true)]
#[case("shell: Bash\n", ExpectedSelection::Rejected, true)]
#[case("shell: /bin/bash\n", ExpectedSelection::Rejected, true)]
#[case("shell: \"{{ shell }}\"\n", ExpectedSelection::Rejected, true)]
#[case("shell: !!str true\n", ExpectedSelection::Rejected, true)]
#[case("shell: ! yes\n", ExpectedSelection::Rejected, true)]
#[case("shell: !!str bash\n", ExpectedSelection::Named("bash"), true)]
#[case("shell: !custom yes\n", ExpectedSelection::PlatformDefault, true)]
#[case("shell: !!bool true\n", ExpectedSelection::Rejected, false)]
#[case("shell: .inf\n", ExpectedSelection::Rejected, false)]
fn front_end_parity(
    #[case] document: &str,
    #[case] expected: ExpectedSelection,
    #[case] front_end_value_parses: bool,
) {
    let front_end_value = serde_saphyr::from_str::<Value>(document);
    assert_eq!(
        front_end_value.is_ok(),
        front_end_value_parses,
        "unexpected serde-saphyr Value parsing for {document:?}"
    );
    let front_end = front_end_value
        .map_err(|error| error.to_string())
        .and_then(|value| {
            serde_json::from_value::<Probe>(value)
                .map(|probe| probe.shell)
                .map_err(|error| error.to_string())
        });
    let direct = serde_saphyr::from_str::<Probe>(document)
        .map(|probe| probe.shell)
        .map_err(|error| error.to_string());
    assert!(
        matches_expected(&front_end, expected),
        "front-end result {front_end:?} did not match {expected:?} for {document:?}"
    );
    assert!(
        matches_expected(&direct, expected),
        "direct result {direct:?} did not match {expected:?} for {document:?}"
    );
    assert_eq!(
        front_end.as_ref().ok(),
        direct.as_ref().ok(),
        "front-end paths diverged for {document:?}"
    );
}

#[test]
fn snapshots_name_and_selector_error_text() {
    let errors = [
        ShellNameError::Empty,
        ShellNameError::TooLong { length: 64 },
        ShellNameError::InvalidFirstCharacter { character: 'B' },
        ShellNameError::InvalidCharacter {
            character: '/',
            char_index: 2,
        },
        ShellNameError::ReservedWord { word: "true" },
    ];
    let mut lines = errors
        .iter()
        .map(|error| format!("{error:?}: {error}"))
        .collect::<Vec<_>>();
    let number_error =
        serde_json::from_value::<ShellSelection>(json!(1)).expect_err("a number must be rejected");
    lines.push(format!("number: {number_error}"));
    let rendered = lines.join("\n");
    crate::snapshot_test_support::snapshot_settings("shell_selection").bind(|| {
        insta::assert_snapshot!("selection_error_texts", rendered);
    });
}
